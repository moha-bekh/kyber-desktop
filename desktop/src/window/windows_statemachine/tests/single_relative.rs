// Project Kyber: single_relative.rs
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
fn single_relative() {
    let _ = env_logger::try_init();

    let mut seq = Sequence::new();

    // Create InputPipeline
    let pipeline_config = kynput_rs::Config::default_dummy_client();
    let pipeline = kynput_rs::Pipeline::new(pipeline_config);

    let input_producer = Arc::new(InputProducer::new(pipeline.get_input_sink()));
    pipeline
        .plug_handler(kynput_rs::create_handler_instance(input_producer.clone()))
        .unwrap();

    // Create mocks
    let mut mock_input_network = MockInputNetwork::new();
    let mock_eventloop_proxy = MockEventLoopProxy::new();

    let mut window = MockWindowControl::new();
    let window_id = 0;
    let display_id = 0;

    // Mock requirements: Window creation
    window.expect_id().returning(move || window_id.into());

    window.expect_inner_size().returning(|| Size {
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

    // Mock requirements: MousePosition
    let mouse_position = kynput_rs::MousePosition {
        display_id: 0,
        x: 960,
        y: 540,
    };

    let mouse_position = mouse_position.to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(mouse_position))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Mock requirements: Initial cursor
    window
        .expect_set_cursor_visible()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window
        .expect_set_cursor_rgba()
        .with(predicate::eq(kynput_rs::Cursor {
            display_id: 0,
            visible: true,
            width: 1,
            height: 1,
            xhot: 0,
            yhot: 0,
            x: 960,
            y: 540,
            data: kynput_rs::CursorData::Raw(vec![0x43434343]),
        }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Set mock requirements: Hide cursor
    window
        .expect_set_cursor_visible()
        .with(predicate::eq(false))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window
        .expect_set_cursor_grab()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Set mock requirements: MouseMove
    let mousemove = kynput_rs::MouseMove { dx: 1, dy: -1 };
    let mousemove_pkt = mousemove.to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(mousemove_pkt))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Set mock requirements: Restore cursor
    window
        .expect_set_cursor_visible()
        .with(predicate::eq(true))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    window
        .expect_set_cursor_rgba()
        .with(predicate::eq(kynput_rs::Cursor {
            display_id: 0,
            visible: true,
            width: 1,
            height: 1,
            xhot: 0,
            yhot: 0,
            x: 480,
            y: 270,
            data: kynput_rs::CursorData::Raw(vec![0x43434343]),
        }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

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

    window
        .expect_set_cursor_position()
        .with(predicate::eq(Position {
            x: 225.0,
            y: 173.4375,
        }))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(()));

    // Set mock requirements: MousePosition
    let mouse_position = kynput_rs::MousePosition {
        display_id: 0,
        x: 1066,
        y: 753,
    };

    let mouse_position = mouse_position.to_packet(InputTarget::Host);

    mock_input_network
        .expect_consume()
        .with(predicate::eq(mouse_position.clone()))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Create InputContext
    let pipeline_network = kynput_rs::create_consumer_instance(Arc::new(mock_input_network));
    pipeline.plug_network(&pipeline_network).unwrap();
    pipeline.start().unwrap();

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

    // Send MousePosition to be inside the window
    windows
        .cursor_moved(window_id.into(), Position { x: 450.0, y: 300.0 })
        .unwrap();

    windows
        .set_cursor(kynput_rs::Cursor {
            display_id: 0,
            visible: true,
            width: 1,
            height: 1,
            xhot: 0,
            yhot: 0,
            x: 960,
            y: 540,
            data: kynput_rs::CursorData::Raw(vec![0x43434343]),
        })
        .unwrap();

    // Send MouseMove. Must be ignored
    input_producer
        .consumer
        .consume(kynput_rs::MouseMove { dx: 1, dy: -1 }.to_packet(InputTarget::Host));

    // Hide cursor to trigger relative mode
    windows
        .set_cursor(kynput_rs::Cursor {
            display_id: 0,
            visible: false,
            width: 0,
            height: 0,
            xhot: 0,
            yhot: 0,
            x: 0,
            y: 0,
            data: kynput_rs::CursorData::None,
        })
        .unwrap();

    // Send MouseMove and MousePosition
    input_producer
        .consumer
        .consume(kynput_rs::MouseMove { dx: 1, dy: -1 }.to_packet(InputTarget::Host));

    windows
        .cursor_moved(0.into(), Position { x: 100.0, y: 200.0 })
        .unwrap();

    // Restore cursor
    windows
        .set_cursor(kynput_rs::Cursor {
            display_id: 0,
            visible: true,
            width: 1,
            height: 1,
            xhot: 0,
            yhot: 0,
            x: 480,
            y: 270,
            data: kynput_rs::CursorData::Raw(vec![0x43434343]),
        })
        .unwrap();

    // Send MouseMove and MousePosition
    windows
        .cursor_moved(window_id.into(), Position { x: 500.0, y: 400.0 })
        .unwrap();

    input_producer
        .consumer
        .consume(kynput_rs::MouseMove { dx: 1, dy: -1 }.to_packet(InputTarget::Host));
}
