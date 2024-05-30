use super::*;

#[test]
fn single_keypress() {
    let _ = env_logger::try_init();

    let mut seq = Sequence::new();

    // Create InputPipeline
    let pipeline_config = libkynput_rs::Config::default_dummy_client();
    let pipeline = libkynput_rs::Pipeline::new(pipeline_config);

    let input_producer = Arc::new(InputProducer::new(pipeline.get_input_sink()));
    pipeline
        .plug_handler(libkynput_rs::create_handler_instance(
            input_producer.clone(),
        ))
        .unwrap();

    // Create mocks
    let mut window = MockWindowControl::new();
    let window_id = 0;
    let display_id = 0;

    let mut mock_input_network = MockInputNetwork::new();
    let mock_eventloop_proxy = MockEventLoopProxy::new();

    // Set mock requirements: Create window
    window.expect_id().returning(move || window_id.into());

    window
        .expect_inner_size()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| Size {
            width: 900.0,
            height: 600.0,
        });

    // Mock requirements: Resized
    window
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| false);

    window
        .expect_set_cursor_rect()
        .with(predicate::eq(None))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window
        .expect_set_cursor_grab()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Mock requirements: KeyboardKey
    let key = libkynput_rs::KeyboardKey {
        pressed: true,
        scancode: 43,
    };

    let key_pkt = key.to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(key_pkt.clone()))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Focus lost
    let flush_pkt = libkynput_rs::ControlEvent::FlushKeys.to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(flush_pkt))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window
        .expect_set_cursor_rect()
        .with(predicate::eq(None))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window
        .expect_set_cursor_grab()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| false);

    // Mock requirements: KeyboardKey
    mock_input_network
        .expect_consume()
        .with(predicate::eq(key_pkt))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Create InputContext
    let pipeline_network = libkynput_rs::create_consumer_instance(Arc::new(mock_input_network));
    pipeline.plug_network(&pipeline_network).unwrap();
    pipeline.start().unwrap();

    let eventloop_proxy = EventLoopProxy::new(mock_eventloop_proxy);
    let input_ctx = Arc::new(InputContext::new(eventloop_proxy, pipeline).unwrap());

    // Plug window
    let mut windows = WindowsStateMachine::<MockWindowControl>::new();
    windows.set_input_ctx(Arc::downgrade(&input_ctx));

    windows.add_window(
        Window::<MockWindowControl>::new(
            window,
            &Display {
                id: display_id,
                width: 1920,
                height: 1080,
                x: 0,
                y: 0,
            },
        )
        .unwrap(),
    );

    windows.resized(
        window_id.into(),
        Size {
            width: 900.0,
            height: 600.0,
        },
    );

    // Send KeyboardKey
    input_producer.consumer.consume(
        libkynput_rs::KeyboardKey {
            pressed: true,
            scancode: 43,
        }
        .to_packet(InputTarget::Host),
    );

    // Send KeyboardKey without focus
    windows.focused(window_id.into(), false).unwrap();

    input_producer.consumer.consume(
        libkynput_rs::KeyboardKey {
            pressed: false,
            scancode: 43,
        }
        .to_packet(InputTarget::Host),
    );

    // Restore focus
    windows.focused(window_id.into(), true).unwrap();

    input_producer.consumer.consume(
        libkynput_rs::KeyboardKey {
            pressed: true,
            scancode: 43,
        }
        .to_packet(InputTarget::Host),
    );
}
