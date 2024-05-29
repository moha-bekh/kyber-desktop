use std::sync::Arc;

use log::{info, warn};
use winit::{
    event::{DeviceEvent, WindowEvent},
    event_loop::EventLoopProxy,
};

use libkynput_rs::{InputType, MouseButton, Packet, Pipeline, RouterProcessingState};

use crate::error::Result;
use crate::inputs::{self, ClientPipelineHook, PipelineStateReader, WinitHandler};
use crate::video_layout::Position;
use crate::UserEvent;

pub(crate) struct InputContext {
    pipeline: Pipeline,
    state_reader: PipelineStateReader,
    winit_handler: Option<Arc<WinitHandler>>,
    supports_relative_mouse: bool,
}

impl InputContext {
    pub(crate) fn new(
        eventloop_proxy: EventLoopProxy<UserEvent>,
        pipeline: Pipeline,
    ) -> Result<Self> {
        // Create InputPipeline
        let cursor_handler = inputs::new_cursor_handler(eventloop_proxy.clone())?;

        pipeline.plug_handler(cursor_handler)?;

        let missing_types: Vec<InputType> = WinitHandler::producable_types()
            .iter()
            .filter(|&&t| !pipeline.produces_type(t))
            .copied()
            .collect();

        let winit_handler = if !missing_types.is_empty() {
            info!("Winit will be used to produce types {:?}", missing_types);
            let winit_handler =
                Arc::new(WinitHandler::new(pipeline.get_input_sink(), missing_types)?);

            pipeline.plug_handler(libkynput_rs::create_handler_instance(winit_handler.clone()))?;

            Some(winit_handler)
        } else {
            None
        };

        // Assume that the default cursor is visible at startup
        pipeline.set_type_processing_state(
            InputType::MouseMove,
            RouterProcessingState::from_boolean(false),
        )?;

        let supports_relative_mouse = pipeline.produces_type(InputType::MouseMove);

        // Create Shortcuts handler
        let pipeline_hook = Box::new(ClientPipelineHook::new_from_event_loop(eventloop_proxy));

        let state_reader = pipeline_hook.get_state_reader();
        pipeline.set_hook(pipeline_hook)?;

        // Create InputContext
        Ok(Self {
            pipeline,
            state_reader,
            supports_relative_mouse,
            winit_handler,
        })
    }

    pub(crate) fn pipeline(&self) -> Pipeline {
        self.pipeline.clone()
    }

    fn mouse_button_filter_pressed(pkt: &Packet) -> bool {
        match MouseButton::from_packet(pkt) {
            Some(mouse_button) => !mouse_button.pressed,
            None => {
                warn!("Expected a MouseButton, got {:?}", pkt.get_type());
                false
            }
        }
    }

    pub(crate) fn cursor_inside_window(&self, inside: bool) -> Result<()> {
        let types = [InputType::MouseMove, InputType::MouseWheel];
        for t in types {
            self.pipeline
                .set_type_processing_state(t, RouterProcessingState::from_boolean(inside))?;
        }

        if inside {
            self.pipeline.set_type_processing_state(
                InputType::MouseButton,
                RouterProcessingState::from_boolean(true),
            )?;
        } else {
            self.pipeline.set_type_processing_state(
                InputType::MouseButton,
                RouterProcessingState::from_callback(Box::new(Self::mouse_button_filter_pressed)),
            )?;
        }

        Ok(())
    }

    pub(crate) fn send_mouse_position(&self, display_id: u32, position: Position) {
        let Some(winit_handler) = self.winit_handler.as_ref() else {
            return;
        };

        winit_handler.send_mouse_position(display_id, position.x as i16, position.y as i16);
    }

    pub(crate) fn device_event(&self, event: &DeviceEvent) {
        let Some(winit_handler) = self.winit_handler.as_ref() else {
            return;
        };

        winit_handler.consume_device_event(event);
    }

    pub(crate) fn window_event(&self, event: &WindowEvent, scale_factor: f64) {
        let Some(winit_handler) = self.winit_handler.as_ref() else {
            return;
        };

        winit_handler.consume_window_event(event, scale_factor);
    }

    pub(crate) fn set_mouse_relative(&self, relative: bool) -> Result<()> {
        if !self.supports_relative_mouse {
            return Ok(());
        }

        self.pipeline.set_type_processing_state(
            InputType::MousePosition,
            RouterProcessingState::from_boolean(!relative),
        )?;

        self.pipeline.set_type_processing_state(
            InputType::MouseMove,
            RouterProcessingState::from_boolean(relative),
        )?;

        Ok(())
    }

    pub(crate) fn flush_keys(&self) -> Result<()> {
        self.pipeline.flush_keys()?;

        Ok(())
    }

    pub(crate) fn set_global_processing_state(&self, process: bool) -> Result<()> {
        self.pipeline.set_global_processing_state(process)?;

        Ok(())
    }

    pub(crate) fn has_one_mouse_button_pressed(&self) -> bool {
        self.state_reader.has_one_mouse_button_pressed()
    }
}
