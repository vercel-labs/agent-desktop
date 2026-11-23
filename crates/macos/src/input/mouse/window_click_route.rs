use agent_desktop_core::{AdapterError, Point};
use core_graphics::{
    event::{CGEvent, CGEventType, EventField},
    geometry::CGPoint,
    sys::CGEventRef,
};
use foreign_types::ForeignType;
use std::{
    ffi::{CStr, c_void},
    ptr::NonNull,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

type SetEventField = unsafe extern "C" fn(CGEventRef, u32, i64);
type SetWindowLocation = unsafe extern "C" fn(CGEventRef, CGPoint);

const EVENT_WINDOW_NUMBER: u32 = 51;
const EVENT_CLICK_GROUP: u32 = 58;
const WINDOW_POINTER_SUBTYPE: i64 = 3;

pub(super) struct WindowClickRoute {
    pid: i32,
    window_number: i64,
    window_point: CGPoint,
    click_group: i64,
    set_field: SetEventField,
    set_window_location: SetWindowLocation,
}

impl WindowClickRoute {
    pub(super) fn new(pid: i32, window_number: i64, point: &Point) -> Result<Self, AdapterError> {
        static SYMBOLS: OnceLock<Option<(SetEventField, SetWindowLocation)>> = OnceLock::new();
        let &(set_field, set_window_location) = SYMBOLS
            .get_or_init(load_symbols)
            .as_ref()
            .ok_or_else(|| AdapterError::not_supported("macOS window-relative mouse clicks"))?;
        let click_group = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|time| i64::from(time.subsec_nanos()))
            .unwrap_or_default();
        Ok(Self {
            pid,
            window_number,
            window_point: CGPoint::new(point.x, point.y),
            click_group,
            set_field,
            set_window_location,
        })
    }

    pub(super) fn post(&self, event: &CGEvent) {
        let phase = if matches!(event.get_type(), CGEventType::MouseMoved) {
            2
        } else {
            3
        };
        let fields = [
            (EventField::MOUSE_EVENT_NUMBER, phase),
            (
                EventField::MOUSE_EVENT_CLICK_STATE,
                event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
            ),
            (
                EventField::MOUSE_EVENT_BUTTON_NUMBER,
                event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER),
            ),
            (EventField::MOUSE_EVENT_SUB_TYPE, WINDOW_POINTER_SUBTYPE),
            (
                EventField::EVENT_TARGET_UNIX_PROCESS_ID,
                i64::from(self.pid),
            ),
            (EVENT_WINDOW_NUMBER, self.window_number),
            (EVENT_CLICK_GROUP, self.click_group),
            (
                EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER,
                self.window_number,
            ),
            (
                EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
                self.window_number,
            ),
        ];
        let raw = event.as_ptr();
        unsafe {
            (self.set_window_location)(raw, self.window_point);
            for (field, value) in fields {
                (self.set_field)(raw, field, value);
            }
        }
        event.post_to_pid(self.pid);
    }
}

fn load_symbols() -> Option<(SetEventField, SetWindowLocation)> {
    let library = unsafe {
        libc::dlopen(
            c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight".as_ptr(),
            libc::RTLD_LAZY | libc::RTLD_LOCAL,
        )
    };
    let library = NonNull::new(library)?;
    let set_field = symbol(library, c"SLEventSetIntegerValueField")?;
    let set_location = symbol(library, c"CGEventSetWindowLocation")?;
    Some(unsafe {
        (
            std::mem::transmute::<*mut c_void, SetEventField>(set_field.as_ptr()),
            std::mem::transmute::<*mut c_void, SetWindowLocation>(set_location.as_ptr()),
        )
    })
}

fn symbol(library: NonNull<c_void>, name: &CStr) -> Option<NonNull<c_void>> {
    NonNull::new(unsafe { libc::dlsym(library.as_ptr(), name.as_ptr()) })
}
