mod cycle;
mod pins;
mod registration;

use core::ffi::{c_char, c_int, c_long, c_void};
use core::{mem, ptr};

use dmc2_hal_sys as hal;

use self::cycle::update_component;
use self::pins::Component;
use self::registration::{publish_initial_values, register_interface};

const COMPONENT_NAME: &[u8] = b"h100_spindle\0";
const FUNCTION_NAME: &[u8] = b"h100-spindle\0";
const _: () = assert!(COMPONENT_NAME.len() - 1 <= hal::HAL_NAME_LEN as usize);
const _: () = assert!(FUNCTION_NAME.len() - 1 <= hal::HAL_NAME_LEN as usize);
const HAL_FAILURE_FORMAT: &[u8] = b"h100_spindle: %s (raw=%d, call=%s): %s; action: %s\n\0";
const COMPONENT_ALLOCATION_FAILURE: &[u8] = b"h100_spindle: HAL_COMPONENT_STATE_ALLOCATION_FAILED (raw=null, call=hal_malloc): HAL shared memory could not hold the spindle component state; action: stop duplicate HAL owners and restore sufficient HAL shared memory before restarting\n\0";
const ENOMEM: c_int = hal::HalKnownErrno::OutOfMemory.raw();
#[cfg(test)]
const EINVAL: c_int = hal::HalKnownErrno::InvalidArgument.raw();

static mut COMPONENT_ID: c_int = -1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupFailure {
    Hal(hal::HalError),
    ComponentAllocation,
}

impl StartupFailure {
    const fn safe_return_code(self) -> c_int {
        match self {
            Self::Hal(error) => error.safe_return_code(),
            Self::ComponentAllocation => ENOMEM,
        }
    }
}

impl From<hal::HalError> for StartupFailure {
    fn from(error: hal::HalError) -> Self {
        Self::Hal(error)
    }
}

unsafe fn exit_component(component_id: c_int) {
    if let Err(error) = hal::HalCall::Exit.classify(unsafe { hal::hal_exit(component_id) }) {
        unsafe { log_hal_failure(error) };
    }
}

unsafe fn log_hal_failure(error: hal::HalError) {
    unsafe {
        hal::rtapi_print_msg(
            hal::msg_level_t_RTAPI_MSG_ERR,
            HAL_FAILURE_FORMAT.as_ptr().cast::<c_char>(),
            error.c_label().as_ptr().cast::<c_char>(),
            error.raw(),
            error.call().c_name().as_ptr().cast::<c_char>(),
            error.c_summary().as_ptr().cast::<c_char>(),
            error.c_action().as_ptr().cast::<c_char>(),
        );
    }
}

unsafe fn log_startup_failure(error: StartupFailure) {
    match error {
        StartupFailure::Hal(error) => unsafe { log_hal_failure(error) },
        StartupFailure::ComponentAllocation => unsafe {
            hal::rtapi_print_msg(
                hal::msg_level_t_RTAPI_MSG_ERR,
                COMPONENT_ALLOCATION_FAILURE.as_ptr().cast::<c_char>(),
            );
        },
    }
}

#[cfg(all(not(debug_assertions), feature = "realtime-component"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    unsafe extern "C" {
        fn abort() -> !;
    }
    unsafe { abort() }
}

#[no_mangle]
pub extern "C" fn rtapi_app_main() -> c_int {
    let component_id = match hal::HalCall::Init
        .classify(unsafe { hal::hal_init(COMPONENT_NAME.as_ptr().cast::<c_char>()) })
    {
        Ok(component_id) => component_id,
        Err(error) => {
            unsafe { log_hal_failure(error) };
            return error.safe_return_code();
        }
    };
    unsafe { COMPONENT_ID = component_id };

    let result = (|| -> Result<(), StartupFailure> {
        let component =
            unsafe { hal::hal_malloc(mem::size_of::<Component>() as c_long) }.cast::<Component>();
        if component.is_null() {
            return Err(StartupFailure::ComponentAllocation);
        }
        unsafe {
            ptr::write(component, Component::new());
            register_interface(component, component_id)?;
            publish_initial_values(&mut *component);
        }

        let exported = unsafe {
            hal::hal_export_funct(
                FUNCTION_NAME.as_ptr().cast::<c_char>(),
                Some(update_component),
                component.cast::<c_void>(),
                1,
                0,
                component_id,
            )
        };
        hal::HalCall::ExportFunct.classify(exported)?;
        hal::HalCall::Ready.classify(unsafe { hal::hal_ready(component_id) })?;
        Ok(())
    })();

    match result {
        Ok(()) => 0,
        Err(error) => {
            unsafe {
                log_startup_failure(error);
                exit_component(component_id);
                COMPONENT_ID = -1;
            }
            error.safe_return_code()
        }
    }
}

#[no_mangle]
pub extern "C" fn rtapi_app_exit() {
    let component_id = unsafe { COMPONENT_ID };
    if component_id >= 0 {
        unsafe {
            exit_component(component_id);
            COMPONENT_ID = -1;
        }
    }
}

#[cfg(test)]
mod tests;
