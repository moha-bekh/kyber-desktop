// Project Kyber: single_mouse_button_outside_video.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0-or-later
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

// A click landing while the cursor is outside the video area must not reach
// the host. The single MousePosition expectation proves the pipeline is live,
// so a leaked MouseButton would fail the test as an unexpected consume.
#[test]
fn single_mouse_button_outside_video() {
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

    // Mock requirements: only the in-video MousePosition reaches the host.
    // No MouseButton expectation: the press sent while outside must be dropped.
    mock_input_network
        .expect_consume()
        .with(predicate::eq(
            kynput_rs::MousePosition {
                display_id: 0,
                x: 960,
                y: 540,
            }
            .to_packet(InputTarget::Host),
        ))
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| ());

    // Create InputContext
    let pipeline_network = kynput_rs::create_consumer_instance(Arc::new(mock_input_network));
    pipeline.plug_network(&pipeline_network).unwrap();
    pipeline.start().unwrap();

    let eventloop_proxy = EventLoopProxy::new(mock_eventloop_proxy);
    let input_ctx = Arc::new(InputContext::new(eventloop_proxy, pipeline).unwrap());

    // Plug window
    let mut windows = WindowsStateMachine::<MockWindowControl>::new(false);
    windows.set_input_ctx(&input_ctx);

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

    // Move inside the video: enables MouseButton, sends the position.
    windows
        .cursor_moved(window_id.into(), Position { x: 450.0, y: 300.0 })
        .unwrap();

    // Move outside the video: disables MouseButton (no button held).
    windows
        .cursor_moved(window_id.into(), Position { x: 450.0, y: 0.0 })
        .unwrap();

    // A press while outside must be dropped by the router.
    input_producer.consumer.consume(
        kynput_rs::MouseButton {
            button: kynput_rs::MouseButtonType::Left,
            pressed: true,
        }
        .to_packet(InputTarget::Host),
    );
}
