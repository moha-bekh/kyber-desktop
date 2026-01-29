// Project Kyber: single_fullscreen_toggle.rs
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
fn single_fullscreen_toggle() {
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

    // Mock requirements: Window creation
    window.expect_id().returning(move || window_id.into());

    window
        .expect_inner_size()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| Size {
            width: 900.0,
            height: 600.0,
        });

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

    // Mock requirements: Enter fullscreen
    window
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| false);

    window
        .expect_monitor_size()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| {
            Some(Size {
                width: 1920.0,
                height: 1080.0,
            })
        });

    window
        .expect_set_fullscreen()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Resized
    window
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| true);

    let lock_rect = Some((
        Position { x: 0.0, y: 0.0 },
        Size {
            width: 1920.0,
            height: 1080.0,
        },
    ));

    window
        .expect_set_cursor_rect()
        .with(predicate::eq(lock_rect))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Mock requirements: Send MousePosition
    let mouse_position = libkynput_rs::MousePosition {
        display_id: 0,
        x: 960,
        y: 540,
    };

    mock_input_network
        .expect_consume()
        .with(predicate::eq(
            mouse_position.clone().to_packet(InputTarget::Host),
        ))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Leave fullscreen
    window
        .expect_is_fullscreen()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| true);

    window
        .expect_set_fullscreen()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

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

    // Mock requirements: Send MousePosition
    mock_input_network
        .expect_consume()
        .with(predicate::eq(mouse_position.to_packet(InputTarget::Host)))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Create InputContext
    let pipeline_network = libkynput_rs::create_consumer_instance(Arc::new(mock_input_network));
    pipeline.plug_network(&pipeline_network).unwrap();

    let eventloop_proxy = EventLoopProxy::new(mock_eventloop_proxy);
    let input_ctx = Arc::new(InputContext::new(eventloop_proxy, pipeline).unwrap());

    // Create window
    let mut windows = WindowsStateMachine::<MockWindowControl>::new(false);
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

    // Enter fullscreen
    windows.toggle_fullscreen(fake_event_loop_target());

    windows.resized(
        window_id.into(),
        Size {
            width: 1920.0,
            height: 1080.0,
        },
    );

    // Send Position inside the video
    windows
        .cursor_moved(window_id.into(), Position { x: 960.0, y: 540.0 })
        .unwrap();

    // Leave fullscreen
    windows.toggle_fullscreen(fake_event_loop_target());

    windows.resized(
        window_id.into(),
        Size {
            width: 900.0,
            height: 600.0,
        },
    );

    // Send Position inside the video
    windows
        .cursor_moved(window_id.into(), Position { x: 450.0, y: 300.0 })
        .unwrap();
}
