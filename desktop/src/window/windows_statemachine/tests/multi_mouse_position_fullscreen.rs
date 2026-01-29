// Project Kyber: multi_mouse_position_fullscreen.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0
//
// This file is both under dual license: AGPLv3 and a Commercial one.
//
// ----
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use super::*;

#[test]
fn multi_mouse_position_fullscreen() {
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

    // Mock requirements: MousePosition in Window 2
    window2
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| true);

    window2
        .expect_gain_focus()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| ());

    let mouse_position = libkynput_rs::MousePosition {
        display_id: window2_display,
        x: 300,
        y: 450,
    }
    .to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(mouse_position))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Create InputContext
    let pipeline_network = libkynput_rs::create_consumer_instance(Arc::new(mock_input_network));
    pipeline.plug_network(&pipeline_network).unwrap();

    let eventloop_proxy = EventLoopProxy::new(mock_eventloop_proxy);
    let input_ctx = Arc::new(InputContext::new(eventloop_proxy, pipeline).unwrap());

    // Create windows
    let mut windows = WindowsStateMachine::<MockWindowControl>::new(false);
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
    windows.toggle_fullscreen(fake_event_loop_target());

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

    // Send Position inside window 2
    windows
        .cursor_moved(window2_id.into(), Position { x: 300.0, y: 450.0 })
        .unwrap();

    // Send MouseMove
    input_producer
        .consumer
        .consume(libkynput_rs::MouseMove { dx: 1, dy: -1 }.to_packet(InputTarget::Host));
}
