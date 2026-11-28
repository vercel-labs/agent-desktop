#[cfg(target_os = "macos")]
mod imp {
    use crate::{
        tree::properties::cf_type::created_cf_array,
        tree::{ax_element::AXElement, properties::ax_value},
    };

    const MALFORMED_AX_VALUE: i32 = i32::MIN;
    use accessibility_sys::kAXErrorSuccess;
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };

    pub fn set_messaging_timeout(
        el: &AXElement,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<(), agent_desktop_core::AdapterError> {
        crate::tree::properties::ax_ipc::prepare(el, deadline).map(|_| ())
    }

    pub fn copy_bool_attr(
        el: &AXElement,
        attr: &str,
        deadline: agent_desktop_core::Deadline,
    ) -> Option<bool> {
        copy_bool_attr_result(el, attr, deadline).ok().flatten()
    }

    pub(crate) fn copy_bool_attr_result(
        el: &AXElement,
        attr: &str,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<Option<bool>, i32> {
        let cf_attr = CFString::new(attr);
        let (err, value) = crate::tree::properties::ax_ipc::copy_attribute_value(
            el,
            cf_attr.as_concrete_TypeRef(),
            deadline,
        );
        if err != kAXErrorSuccess {
            if !value.is_null() {
                unsafe { core_foundation::base::CFRelease(value) };
            }
            return if is_absent_error(err) {
                Ok(None)
            } else {
                Err(err)
            };
        }
        if value.is_null() {
            return Ok(None);
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type
            .downcast::<CFBoolean>()
            .map(|value| Some(value.into()))
            .ok_or(MALFORMED_AX_VALUE)
    }

    pub(crate) fn copy_ax_array_prefix_result(
        el: &AXElement,
        attr: &str,
        max_values: usize,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<Option<Vec<AXElement>>, i32> {
        if max_values == 0 {
            return Ok(Some(Vec::new()));
        }
        let cf_attr = CFString::new(attr);
        let (err, value) = crate::tree::properties::ax_ipc::copy_attribute_values(
            el,
            cf_attr.as_concrete_TypeRef(),
            0,
            max_values as core_foundation_sys::base::CFIndex,
            deadline,
        );
        if err != kAXErrorSuccess {
            if !value.is_null() {
                drop(created_cf_array(value as CFTypeRef));
            }
            if err == accessibility_sys::kAXErrorFailure {
                let (fallback_err, fallback_val) =
                    crate::tree::properties::ax_ipc::copy_attribute_value(
                        el,
                        cf_attr.as_concrete_TypeRef(),
                        deadline,
                    );
                if fallback_err == kAXErrorSuccess && !fallback_val.is_null() {
                    let Some(full_arr) = created_cf_array(fallback_val) else {
                        return Err(MALFORMED_AX_VALUE);
                    };
                    let total = full_arr.len() as usize;
                    let elements = ax_array_items(full_arr);
                    if elements.len() != total {
                        return Err(MALFORMED_AX_VALUE);
                    }
                    let slice = elements.into_iter().take(max_values).collect();
                    return Ok(Some(slice));
                }
            }
            return if is_absent_error(err) {
                Ok(None)
            } else {
                Err(err)
            };
        }
        if value.is_null() {
            return Ok(None);
        }
        let Some(arr) = created_cf_array(value as CFTypeRef) else {
            return Err(MALFORMED_AX_VALUE);
        };
        let expected = arr.len() as usize;
        let elements = ax_array_items(arr);
        if elements.len() != expected {
            return Err(MALFORMED_AX_VALUE);
        }
        Ok(Some(elements))
    }

    pub(crate) fn copy_element_attr_result(
        el: &AXElement,
        attr: &str,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<Option<AXElement>, i32> {
        copy_element_attr_with(el, attr, deadline, is_absent_error)
    }

    /// Like `copy_element_attr_result`, but `Ok(None)` only when the element
    /// answered that the attribute has no value. An attribute it does not
    /// implement or fails to answer is `Err`, so "none" is never a guess.
    pub(crate) fn copy_element_attr_answered_result(
        el: &AXElement,
        attr: &str,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<Option<AXElement>, i32> {
        copy_element_attr_with(el, attr, deadline, |error| {
            error == accessibility_sys::kAXErrorNoValue
        })
    }

    fn copy_element_attr_with(
        el: &AXElement,
        attr: &str,
        deadline: agent_desktop_core::Deadline,
        absent: impl Fn(i32) -> bool,
    ) -> Result<Option<AXElement>, i32> {
        let cf_attr = CFString::new(attr);
        let (err, value) = crate::tree::properties::ax_ipc::copy_attribute_value(
            el,
            cf_attr.as_concrete_TypeRef(),
            deadline,
        );
        if err != kAXErrorSuccess {
            if !value.is_null() {
                unsafe { core_foundation::base::CFRelease(value) };
            }
            return if absent(err) { Ok(None) } else { Err(err) };
        }
        if value.is_null() {
            return Ok(None);
        }
        ax_value::created_ax_element(value)
            .map(Some)
            .ok_or(MALFORMED_AX_VALUE)
    }

    pub(crate) fn selected_text_range(
        element: &AXElement,
        deadline: agent_desktop_core::Deadline,
    ) -> Option<std::ops::Range<usize>> {
        use accessibility_sys::{
            AXValueGetTypeID, AXValueGetValue, kAXErrorSuccess, kAXValueTypeCFRange,
        };
        use core_foundation::{
            base::{CFType, TCFType},
            string::CFString,
        };
        use core_foundation_sys::base::CFRange;

        let attribute = CFString::new("AXSelectedTextRange");
        let (error, raw) = crate::tree::properties::ax_ipc::copy_attribute_value(
            element,
            attribute.as_concrete_TypeRef(),
            deadline,
        );
        if raw.is_null() {
            return None;
        }
        let value = unsafe { CFType::wrap_under_create_rule(raw) };
        if error != kAXErrorSuccess || value.type_of() != unsafe { AXValueGetTypeID() } {
            return None;
        }
        let mut range = CFRange {
            location: 0,
            length: 0,
        };
        if !unsafe {
            AXValueGetValue(
                raw as _,
                kAXValueTypeCFRange,
                (&mut range as *mut CFRange).cast(),
            )
        } {
            return None;
        }
        let start = usize::try_from(range.location).ok()?;
        let length = usize::try_from(range.length).ok()?;
        Some(start..start.checked_add(length)?)
    }

    /// Selects `range`, in UTF-16 units, in a text element. True when the
    /// application acknowledged the write; the caller reads the selection
    /// back, because an acknowledged write can be ignored.
    pub(crate) fn set_selected_text_range(
        element: &AXElement,
        range: std::ops::Range<usize>,
        deadline: agent_desktop_core::Deadline,
    ) -> bool {
        use accessibility_sys::{AXValueCreate, kAXValueTypeCFRange};
        use core_foundation::base::TCFType;
        use core_foundation_sys::base::CFRange;

        let (Ok(location), Ok(length)) =
            (isize::try_from(range.start), isize::try_from(range.len()))
        else {
            return false;
        };
        let range = CFRange { location, length };
        let value =
            unsafe { AXValueCreate(kAXValueTypeCFRange, (&range as *const CFRange).cast()) };
        if value.is_null() {
            return false;
        }
        let value = unsafe { CFType::wrap_under_create_rule(value as _) };
        let attribute = CFString::new("AXSelectedTextRange");
        crate::tree::properties::ax_ipc::set_attribute_value(
            element,
            attribute.as_concrete_TypeRef(),
            value.as_CFTypeRef(),
            deadline,
        )
        .is_ok_and(|error| error == kAXErrorSuccess)
    }

    fn ax_array_items(arr: CFArray<CFType>) -> Vec<AXElement> {
        arr.into_iter()
            .filter_map(|item| ax_value::retained_ax_element(&item))
            .collect()
    }

    fn is_absent_error(error: i32) -> bool {
        crate::tree::properties::ax_absence::is_absent_attribute_error(error)
    }
}

pub(crate) use super::text_attributes::{
    copy_string_attr_bounded_result, copy_string_attr_result, copy_value_typed,
    copy_value_typed_bounded_result, copy_value_typed_result,
};
pub(crate) use imp::{
    copy_ax_array_prefix_result, copy_bool_attr, copy_bool_attr_result,
    copy_element_attr_answered_result, copy_element_attr_result, set_messaging_timeout,
};

#[cfg(target_os = "macos")]
pub(crate) use imp::{selected_text_range, set_selected_text_range};
