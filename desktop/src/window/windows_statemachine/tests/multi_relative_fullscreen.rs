use super::*;

#[test]
fn multi_relative_fullscreen() {
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
    let mut window1 = MockWindowControl::new();
    let window1_id = 0;
    let window1_display = 0;

    let mut window2 = MockWindowControl::new();
    let window2_id = 1;
    let window2_display = 1;

    let mut mock_input_network = MockInputNetwork::new();
    let mock_eventloop_proxy = MockEventLoopProxy::new();

    // Mock requirements: Window 1 creation
    window1.expect_id().returning(move || window1_id.into());

    window1.expect_inner_size().returning(|| Size {
        width: 900.0,
        height: 600.0,
    });

    // Mock requirements: Window 1 resized
    // Still one window
    window1
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| false);

    window1
        .expect_set_cursor_rect()
        .with(predicate::eq(None))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window1
        .expect_set_cursor_grab()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Mock requirements: Window 2 creation
    // Resized handler has a different behavior when there are multiple
    // windows
    window2.expect_id().returning(move || window2_id.into());

    window2.expect_inner_size().returning(|| Size {
        width: 900.0,
        height: 600.0,
    });

    // Mock requirements: Window 1 enters fullscreen
    window1
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| false);

    window1
        .expect_monitor_size()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| {
            Some(Size {
                width: 1920.0,
                height: 1080.0,
            })
        });

    window1
        .expect_set_fullscreen()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Window 2 enters fullscreen
    window2
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| false);

    window2
        .expect_monitor_size()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| {
            Some(Size {
                width: 1920.0,
                height: 1080.0,
            })
        });

    window2
        .expect_set_fullscreen()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: MousePosition in Window 1
    window1
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| true);

    window1
        .expect_gain_focus()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| ());

    let mouse_position = libkynput_rs::MousePosition {
        display_id: window1_display,
        x: 450,
        y: 300,
    }
    .to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(mouse_position))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Set cursor
    window1
        .expect_set_cursor_visible()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window1
        .expect_set_cursor_rgba()
        .with(predicate::eq(libkynput_rs::Cursor {
            display_id: window1_display,
            visible: true,
            x: 0,
            y: 0,
            xhot: 0,
            yhot: 0,
            width: 1,
            height: 1,
            data: libkynput_rs::CursorData::Raw(vec![0x43434343]),
        }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Hide cursor
    window1
        .expect_set_cursor_visible()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window2
        .expect_set_cursor_visible()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window1
        .expect_set_cursor_grab()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Mock requirements: MouseMove
    let mousemove = libkynput_rs::MouseMove { dx: 1, dy: -1 }.to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(mousemove))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Restore cursor on Window 1
    window1
        .expect_set_cursor_visible()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window1
        .expect_set_cursor_rgba()
        .with(predicate::eq(libkynput_rs::Cursor {
            display_id: window1_display,
            visible: true,
            x: 960,
            y: 540,
            xhot: 0,
            yhot: 0,
            width: 1,
            height: 1,
            data: libkynput_rs::CursorData::Raw(vec![0x43434343]),
        }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window1
        .expect_set_cursor_rect()
        .with(predicate::eq(None))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window1
        .expect_set_cursor_grab()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window1
        .expect_set_cursor_position()
        .with(predicate::eq(Position { x: 960.0, y: 540.0 }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Mock requirements: Hide cursor
    window1
        .expect_set_cursor_visible()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window2
        .expect_set_cursor_visible()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window1
        .expect_set_cursor_grab()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Mock requirements: MouseMove
    let mousemove = libkynput_rs::MouseMove { dx: 2, dy: -1 }.to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(mousemove))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Restore cursor on Window 2
    window2
        .expect_set_cursor_visible()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window2
        .expect_set_cursor_rgba()
        .with(predicate::eq(libkynput_rs::Cursor {
            display_id: window2_display,
            visible: true,
            x: 960,
            y: 540,
            xhot: 0,
            yhot: 0,
            width: 1,
            height: 1,
            data: libkynput_rs::CursorData::Raw(vec![0x43434343]),
        }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window1
        .expect_set_cursor_rect()
        .with(predicate::eq(None))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window1
        .expect_set_cursor_grab()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    window2
        .expect_set_cursor_position()
        .with(predicate::eq(Position { x: 960.0, y: 540.0 }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Create InputContext
    let pipeline_network = libkynput_rs::create_consumer_instance(Arc::new(mock_input_network));
    pipeline.plug_network(&pipeline_network).unwrap();

    let eventloop_proxy = EventLoopProxy::new(mock_eventloop_proxy);
    let input_ctx = Arc::new(InputContext::new(eventloop_proxy, pipeline).unwrap());

    // Create windows
    let mut windows = WindowsStateMachine::<MockWindowControl>::new();
    windows.set_input_ctx(Arc::downgrade(&input_ctx));

    // Window 1
    windows.add_window(
        Window::<MockWindowControl>::new(
            window1,
            &Display {
                id: window1_display,
                width: 1920,
                height: 1080,
                x: 0,
                y: 0,
            },
        )
        .unwrap(),
    );

    windows.resized(
        window1_id.into(),
        Size {
            width: 900.0,
            height: 600.0,
        },
    );

    // Window 2
    windows.add_window(
        Window::<MockWindowControl>::new(
            window2,
            &Display {
                id: window2_display,
                width: 1920,
                height: 1080,
                x: 0,
                y: 0,
            },
        )
        .unwrap(),
    );

    windows.resized(
        window2_id.into(),
        Size {
            width: 900.0,
            height: 600.0,
        },
    );

    // Toggle fullscreen
    windows.toggle_fullscreen();

    windows.resized(
        window1_id.into(),
        Size {
            width: 1920.0,
            height: 1080.0,
        },
    );

    windows.resized(
        window2_id.into(),
        Size {
            width: 1920.0,
            height: 1080.0,
        },
    );

    // Send Position inside window 1
    windows
        .cursor_moved(window1_id.into(), Position { x: 450.0, y: 300.0 })
        .unwrap();

    windows
        .set_cursor(libkynput_rs::Cursor {
            display_id: window1_display,
            visible: true,
            x: 0,
            y: 0,
            xhot: 0,
            yhot: 0,
            width: 1,
            height: 1,
            data: libkynput_rs::CursorData::Raw(vec![0x43434343]),
        })
        .unwrap();

    windows
        .set_cursor(libkynput_rs::Cursor {
            display_id: window2_display,
            visible: false,
            x: 0,
            y: 0,
            xhot: 0,
            yhot: 0,
            width: 0,
            height: 0,
            data: libkynput_rs::CursorData::None,
        })
        .unwrap();

    // Hide cursor
    windows
        .set_cursor(libkynput_rs::Cursor {
            display_id: window1_display,
            visible: false,
            x: 0,
            y: 0,
            xhot: 0,
            yhot: 0,
            width: 0,
            height: 0,
            data: libkynput_rs::CursorData::None,
        })
        .unwrap();

    input_producer
        .consumer
        .consume(libkynput_rs::MouseMove { dx: 1, dy: -1 }.to_packet(InputTarget::Host));

    // Restore cursor on Window 1
    windows
        .set_cursor(libkynput_rs::Cursor {
            display_id: window1_display,
            visible: true,
            x: 960,
            y: 540,
            xhot: 0,
            yhot: 0,
            width: 1,
            height: 1,
            data: libkynput_rs::CursorData::Raw(vec![0x43434343]),
        })
        .unwrap();

    // Hide cursor
    windows
        .set_cursor(libkynput_rs::Cursor {
            display_id: window1_display,
            visible: false,
            x: 0,
            y: 0,
            xhot: 0,
            yhot: 0,
            width: 0,
            height: 0,
            data: libkynput_rs::CursorData::None,
        })
        .unwrap();

    input_producer
        .consumer
        .consume(libkynput_rs::MouseMove { dx: 2, dy: -1 }.to_packet(InputTarget::Host));

    // Restore cursor on Window 2
    windows
        .set_cursor(libkynput_rs::Cursor {
            display_id: window2_display,
            visible: true,
            x: 960,
            y: 540,
            xhot: 0,
            yhot: 0,
            width: 1,
            height: 1,
            data: libkynput_rs::CursorData::Raw(vec![0x43434343]),
        })
        .unwrap();
}
