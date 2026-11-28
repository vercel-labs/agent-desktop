pub(crate) mod envelope_out;
pub(crate) mod execute_by_ref;
pub(crate) mod execute_by_ref_timeout;
pub(crate) mod snapshot;
pub(crate) mod status;
pub(crate) mod timeout;
pub(crate) mod trace_export;
pub(crate) mod trace_show;
pub(crate) mod version;
pub(crate) mod wait;

macro_rules! command_scope {
    ($context:expr, $name:expr) => {{
        match $context.command_scope($name) {
            Ok(scope) => scope,
            Err(error) => {
                let error = agent_desktop_core::AppError::into_adapter_error(error);
                $crate::error::set_last_error(&error);
                return $crate::error::last_error_code();
            }
        }
    }};
}

macro_rules! mutating_command_scope {
    ($context:expr, $name:expr) => {{
        match $context.mutating_command_scope($name) {
            Ok(scope) => scope,
            Err(error) => {
                let error = agent_desktop_core::AppError::into_adapter_error(error);
                $crate::error::set_last_error(&error);
                return $crate::error::last_error_code();
            }
        }
    }};
}

macro_rules! complete_scope {
    ($scope:expr, $result:expr) => {{
        if let Err(error) = $scope.complete($result) {
            let error = agent_desktop_core::AppError::into_adapter_error(error);
            $crate::error::set_last_error(&error);
            return $crate::error::last_error_code();
        }
    }};
}

pub(crate) use command_scope;
pub(crate) use complete_scope;
pub(crate) use mutating_command_scope;
