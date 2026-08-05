use modbussim_core::master::{
    MasterConfig, MasterConnection, MasterError, MasterState, PollConfig, PollEvent, ReadFunction,
    ReadResult,
};
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::socks5::Socks5Config;
use modbussim_core::transport::Transport;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Helper: start a slave on the given port with a device at slave_id=1.
async fn start_slave(port: u16) -> SlaveConnection {
    let transport = Transport::Tcp {
        host: "127.0.0.1".to_string(),
        port,
    };
    let mut conn = SlaveConnection::new(transport);

    let mut device = SlaveDevice::new(1, "Test Device");
    // Pre-fill addresses 0..30 for all four areas so write-path tests
    // (which target addresses 10 / 20 / etc) don't hit IllegalDataAddress.
    for addr in 0..30u16 {
        device.register_map.holding_registers.insert(addr, 0);
        device.register_map.input_registers.insert(addr, 0);
        device.register_map.coils.insert(addr, false);
        device.register_map.discrete_inputs.insert(addr, false);
    }
    // Seed the values used by the read-path tests.
    device.register_map.write_holding_register(0, 1000);
    device.register_map.write_holding_register(1, 2000);
    device.register_map.write_holding_register(2, 3000);
    device.register_map.write_coil(0, true);
    device.register_map.discrete_inputs.insert(0, true);
    device.register_map.input_registers.insert(0, 500);
    conn.add_device(device).await.unwrap();

    conn.start().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    conn
}

fn master_config(port: u16) -> MasterConfig {
    MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        slave_id: 1,
        timeout_ms: 3000,
        ..Default::default()
    }
}

fn new_master(port: u16) -> MasterConnection {
    let config = master_config(port);
    let transport = Transport::Tcp {
        host: config.target_address.clone(),
        port: config.port,
    };
    MasterConnection::new(config, transport)
}

async fn start_socks5_proxy() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.unwrap();
        let mut greeting = [0u8; 3];
        client.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [0x05, 0x01, 0x00]);
        client.write_all(&[0x05, 0x00]).await.unwrap();

        let mut header = [0u8; 4];
        client.read_exact(&mut header).await.unwrap();
        assert_eq!(&header[..3], &[0x05, 0x01, 0x00]);
        let host = match header[3] {
            0x01 => {
                let mut bytes = [0u8; 4];
                client.read_exact(&mut bytes).await.unwrap();
                std::net::Ipv4Addr::from(bytes).to_string()
            }
            0x03 => {
                let length = client.read_u8().await.unwrap() as usize;
                let mut bytes = vec![0u8; length];
                client.read_exact(&mut bytes).await.unwrap();
                String::from_utf8(bytes).unwrap()
            }
            other => panic!("unexpected SOCKS5 address type {other}"),
        };
        let port = client.read_u16().await.unwrap();
        let mut upstream = TcpStream::connect((host.as_str(), port)).await.unwrap();
        client
            .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 0])
            .await
            .unwrap();
        tokio::io::copy_bidirectional(&mut client, &mut upstream)
            .await
            .unwrap();
    });
    (address, task)
}

#[tokio::test]
async fn test_master_connect_disconnect() {
    let mut slave = start_slave(16001).await;
    let mut master = new_master(16001);

    assert_eq!(master.state(), MasterState::Disconnected);
    master.connect().await.unwrap();
    assert_eq!(master.state(), MasterState::Connected);

    // Double connect should fail
    assert!(master.connect().await.is_err());

    master.disconnect().await.unwrap();
    assert_eq!(master.state(), MasterState::Disconnected);

    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_read_holding_registers() {
    let mut slave = start_slave(16002).await;
    let mut master = new_master(16002);
    master.connect().await.unwrap();

    let result = master
        .read(ReadFunction::ReadHoldingRegisters, 0, 3)
        .await
        .unwrap();
    match result {
        ReadResult::HoldingRegisters(values) => {
            assert_eq!(values, vec![1000, 2000, 3000]);
        }
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_read_through_socks5_proxy() {
    let mut slave = start_slave(16012).await;
    let (proxy_address, proxy_task) = start_socks5_proxy().await;
    let config = MasterConfig {
        target_address: "localhost".to_string(),
        port: 16012,
        slave_id: 1,
        timeout_ms: 3000,
        socks5: Socks5Config {
            enabled: true,
            host: proxy_address.ip().to_string(),
            port: proxy_address.port(),
            ..Default::default()
        },
        ..Default::default()
    };
    let transport = Transport::Tcp {
        host: config.target_address.clone(),
        port: config.port,
    };
    let mut master = MasterConnection::new(config, transport);

    master.connect().await.unwrap();
    let result = master
        .read(ReadFunction::ReadHoldingRegisters, 0, 3)
        .await
        .unwrap();
    match result {
        ReadResult::HoldingRegisters(values) => {
            assert_eq!(values, vec![1000, 2000, 3000]);
        }
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    proxy_task.await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_read_coils() {
    let mut slave = start_slave(16003).await;
    let mut master = new_master(16003);
    master.connect().await.unwrap();

    let result = master.read(ReadFunction::ReadCoils, 0, 2).await.unwrap();
    match result {
        ReadResult::Coils(values) => {
            assert_eq!(values, vec![true, false]);
        }
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_read_discrete_inputs() {
    let mut slave = start_slave(16004).await;
    let mut master = new_master(16004);
    master.connect().await.unwrap();

    let result = master
        .read(ReadFunction::ReadDiscreteInputs, 0, 1)
        .await
        .unwrap();
    match result {
        ReadResult::DiscreteInputs(values) => {
            assert_eq!(values, vec![true]);
        }
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_read_input_registers() {
    let mut slave = start_slave(16005).await;
    let mut master = new_master(16005);
    master.connect().await.unwrap();

    let result = master
        .read(ReadFunction::ReadInputRegisters, 0, 1)
        .await
        .unwrap();
    match result {
        ReadResult::InputRegisters(values) => {
            assert_eq!(values, vec![500]);
        }
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_write_single_coil() {
    let mut slave = start_slave(16006).await;
    let mut master = new_master(16006);
    master.connect().await.unwrap();

    master.write_single_coil(10, true).await.unwrap();

    // Read back
    let result = master.read(ReadFunction::ReadCoils, 10, 1).await.unwrap();
    match result {
        ReadResult::Coils(values) => assert_eq!(values, vec![true]),
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_write_single_register() {
    let mut slave = start_slave(16007).await;
    let mut master = new_master(16007);
    master.connect().await.unwrap();

    master.write_single_register(10, 42).await.unwrap();

    let result = master
        .read(ReadFunction::ReadHoldingRegisters, 10, 1)
        .await
        .unwrap();
    match result {
        ReadResult::HoldingRegisters(values) => assert_eq!(values, vec![42]),
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_write_multiple_coils() {
    let mut slave = start_slave(16008).await;
    let mut master = new_master(16008);
    master.connect().await.unwrap();

    master
        .write_multiple_coils(20, &[true, false, true])
        .await
        .unwrap();

    let result = master.read(ReadFunction::ReadCoils, 20, 3).await.unwrap();
    match result {
        ReadResult::Coils(values) => assert_eq!(values, vec![true, false, true]),
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_write_multiple_registers() {
    let mut slave = start_slave(16009).await;
    let mut master = new_master(16009);
    master.connect().await.unwrap();

    master
        .write_multiple_registers(20, &[111, 222, 333])
        .await
        .unwrap();

    let result = master
        .read(ReadFunction::ReadHoldingRegisters, 20, 3)
        .await
        .unwrap();
    match result {
        ReadResult::HoldingRegisters(values) => assert_eq!(values, vec![111, 222, 333]),
        _ => panic!("unexpected result type"),
    }

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}

#[tokio::test]
async fn test_master_not_connected_error() {
    let master = new_master(16010);
    let result = master.read(ReadFunction::ReadHoldingRegisters, 0, 1).await;
    assert!(matches!(result, Err(MasterError::NotConnected)));
}

#[tokio::test]
async fn test_master_connection_timeout() {
    // Try to connect to a port that nothing is listening on
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port: 19999,
        slave_id: 1,
        timeout_ms: 500,
        ..Default::default()
    };
    let transport = Transport::Tcp {
        host: config.target_address.clone(),
        port: config.port,
    };
    let mut master = MasterConnection::new(config, transport);
    let result = master.connect().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_master_polling() {
    let mut slave = start_slave(16011).await;
    let mut master = new_master(16011);
    master.connect().await.unwrap();

    let poll_config = PollConfig {
        function: ReadFunction::ReadHoldingRegisters,
        start_address: 0,
        quantity: 3,
        interval_ms: 100,
    };

    let mut rx = master.start_poll(poll_config).await.unwrap();
    assert!(master.is_polling());

    // Collect a few poll results
    let mut count = 0;
    while count < 3 {
        if let Some(event) = rx.recv().await {
            match event {
                PollEvent::Data(ReadResult::HoldingRegisters(values)) => {
                    assert_eq!(values, vec![1000, 2000, 3000]);
                    count += 1;
                }
                PollEvent::Error(e) => panic!("unexpected poll error: {e}"),
                _ => panic!("unexpected poll event type"),
            }
        }
    }

    master.stop_poll().await.unwrap();
    assert!(!master.is_polling());

    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();
}
