use std::sync::{Arc, Mutex};

use ddc_hi::{Ddc, DdcHost, Display, Handle};

pub struct Control {
    pub code: u8,
    pub name: &'static str,
}

pub const BRIGHTNESS: Control = Control {
    code: 0x10,
    name: "Brightness",
};
pub const CONTRAST: Control = Control {
    code: 0x12,
    name: "Contrast",
};

// Controller for each value
pub struct Controller {
    pub value: u16,
    handle: Arc<Mutex<Handle>>,
    pub control: Control,
}

impl Controller {
    pub fn new(control: Control, handle: Arc<Mutex<Handle>>) -> Self {
        Self {
            handle,
            control,
            value: 0,
        }
    }

    pub fn get(&mut self) -> Result<u16, <Handle as DdcHost>::Error> {
        let mut handle = self.handle.lock().unwrap();
        let value = handle.get_vcp_feature(self.control.code)?.value();
        self.value = value;
        Ok(value)
    }

    pub fn set(&mut self, value: u16) -> Result<(), <Handle as DdcHost>::Error> {
        let mut handle = self.handle.lock().unwrap();
        handle.set_vcp_feature(self.control.code, value)?;
        self.value = value;
        Ok(())
    }
}

pub struct MyDisplay {
    handle: Arc<Mutex<Handle>>,
    pub name: String,
    pub controls: Vec<Arc<Mutex<Controller>>>,
}

pub struct DisplayManager {
    pub displays: Vec<Arc<MyDisplay>>,
}

impl DisplayManager {
    pub fn new() -> Self {
        Self { displays: vec![] }
    }

    pub fn refresh(&mut self) -> Result<(), <Handle as DdcHost>::Error> {
        self.displays.clear();
        for mut display in Display::enumerate() {
            display.update_capabilities().unwrap();
            let handle = Arc::new(Mutex::new(display.handle));

            let mut brightness = Controller::new(BRIGHTNESS, Arc::clone(&handle));
            let mut contrast = Controller::new(CONTRAST, Arc::clone(&handle));
            brightness.get()?;
            contrast.get()?;

            self.displays.push(Arc::new(MyDisplay {
                name: display.info.model_name.unwrap_or_else(|| {
                    display
                        .info
                        .serial_number
                        .unwrap_or_else(|| display.info.id)
                }),
                controls: vec![
                    Arc::new(Mutex::new(brightness)),
                    Arc::new(Mutex::new(contrast)),
                ],
                handle,
            }));
        }
        Ok(())
    }
}
