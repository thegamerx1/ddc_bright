use ddc_hi::{Ddc, DdcHost, Display, Handle};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::{sync::mpsc::channel, thread};

#[derive(PartialEq, PartialOrd, Eq, Hash, Clone, Copy)]
pub enum Control {
    BRIGHTNESS = 0x10,
    CONTRAST = 0x12,
}

impl Control {
    pub fn get_name(&self) -> &'static str {
        match &self {
            Control::BRIGHTNESS => "Brightness",
            Control::CONTRAST => "Contrast",
        }
    }
}

const ALL_CONTROLS: [Control; 2] = [Control::BRIGHTNESS, Control::CONTRAST];

#[derive(Clone, Copy)]
pub struct Controller {
    pub value: u16,
    pub kind: Control,
}

pub struct MyDisplay {
    handle: Arc<Mutex<Handle>>,
    pub name: String,
    pub controls: HashMap<Control, WrappedController>,
}

impl MyDisplay {
    pub fn new(handle: Handle, name: String) -> Self {
        let mut controls = HashMap::new();
        for control in ALL_CONTROLS {
            controls.insert(
                control,
                Arc::new(RwLock::new(Controller {
                    kind: control,
                    value: 0,
                })),
            );
        }

        Self {
            handle: Arc::new(Mutex::new(handle)),
            name,
            controls,
        }
    }

    fn load(&self) {
        for control in ALL_CONTROLS {
            let value = self.get(control.clone());
            let mut controller = self.controls.get(&control).unwrap().write().unwrap();
            controller.value = value;
        }
    }

    pub fn get(&self, control: Control) -> u16 {
        let mut handle = self.handle.lock().unwrap();
        handle.get_vcp_feature(control as u8).unwrap().value()
    }

    pub fn set(&self, control: Control, value: u16) -> () {
        let mut handle = self.handle.lock().unwrap();
        handle.set_vcp_feature(control as u8, value).unwrap();
    }
}

pub type WrappedDisplay = Arc<MyDisplay>;
pub type WrappedController = Arc<RwLock<Controller>>;

struct Change {
    display: WrappedDisplay,
    controller: Controller,
}

pub struct DisplayManager {
    pub displays: Vec<WrappedDisplay>,
    changes: Arc<Mutex<Vec<Change>>>,
    tx_queue: Sender<()>,
}

impl DisplayManager {
    pub fn new() -> Self {
        let changes = Arc::new(Mutex::new(vec![]));
        let changes_clone = changes.clone();
        let (sender, receiver) = channel::<()>();

        thread::spawn(move || loop {
            if receiver.recv().is_err() {
                return;
            }
            let mut changes = changes_clone.lock().unwrap();
            let change: Change = changes.remove(0);
            drop(changes);

            change
                .display
                .set(change.controller.kind, change.controller.value);
        });

        Self {
            displays: vec![],
            changes,
            tx_queue: sender,
        }
    }

    pub fn queue_change(&self, display: WrappedDisplay, controller: WrappedController, value: i16) {
        match self.tx_queue.send(()) {
            Ok(()) => (),
            Err(err) => {
                eprintln!("{err}");
            }
        }

        let mut control = controller.write().unwrap();
        control.value = max(min(control.value as i16 + value, 100), 0) as u16;

        let mut changes = self.changes.lock().unwrap();
        changes.push(Change {
            display: display,
            controller: control.clone(),
        });
    }

    pub fn refresh(&mut self) -> Result<(), <Handle as DdcHost>::Error> {
        self.displays.clear();
        for display in Display::enumerate() {
            let display = MyDisplay::new(
                display.handle,
                display.info.model_name.unwrap_or_else(|| {
                    display
                        .info
                        .serial_number
                        .unwrap_or_else(|| display.info.id)
                }),
            );

            display.load();

            self.displays.push(Arc::new(display));
        }
        Ok(())
    }
}
