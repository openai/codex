//! Probe PSEC creation and startup attributes without launching a process.
//! Cache the result so automatic MXC selection requires a usable native API.

use std::sync::OnceLock;

use flatbuffers::FlatBufferBuilder;
use mxc_sdk::mxc_common::models::ContainerPolicy;
use mxc_sdk::mxc_common::ui_policy::resolve_ui_restrictions;
use mxc_sdk::process_container_common::PROCESS_SECURITY_ENVIRONMENT_FLAG_NONE;
use mxc_sdk::process_container_common::ProcessSecurityEnvironmentApi;
use mxc_sdk::process_container_common::SecurityEnvironmentStartupInfo;
use mxc_sdk::process_container_common::job_object::to_job_object_uilimit_mask;
use mxc_sdk::process_security_environment_spec::process_security_environment_layout::EndpointPolicyT;
use mxc_sdk::process_security_environment_spec::process_security_environment_layout::FilterAction;
use mxc_sdk::process_security_environment_spec::process_security_environment_layout::NetworkPolicyT;
use mxc_sdk::process_security_environment_spec::process_security_environment_layout::ProcessSecurityEnvironmentT;
use mxc_sdk::process_security_environment_spec::process_security_environment_layout::SchemaVersionT;
use mxc_sdk::process_security_environment_spec::process_security_environment_layout::finish_process_security_environment_buffer;

pub(super) fn is_available() -> bool {
    static USABLE: OnceLock<bool> = OnceLock::new();
    *USABLE.get_or_init(|| {
        // Preserve the previous SDK's probe: no filesystem, network, or UI
        // grants, and no process launch. API-set presence alone is insufficient.
        let policy = ContainerPolicy::default();
        let mut egress = EndpointPolicyT::default();
        egress.default_action = FilterAction::deny;
        let mut network = NetworkPolicyT::default();
        network.egress = Some(Box::new(egress));
        let mut specification = ProcessSecurityEnvironmentT::default();
        specification.version = SchemaVersionT { major: 1, minor: 0 };
        specification.disallow_win32k_system_calls = policy.ui.disable;
        specification.ui_restrictions = u64::from(to_job_object_uilimit_mask(
            &resolve_ui_restrictions(&policy.ui, &policy.base_process_ui),
        ));
        specification.network_policy = Some(Box::new(network));
        let mut builder = FlatBufferBuilder::with_capacity(/* capacity */ 1024);
        let specification = specification.pack(&mut builder);
        finish_process_security_environment_buffer(&mut builder, specification);
        ProcessSecurityEnvironmentApi::load()
            .and_then(|api| {
                api.create(
                    builder.finished_data(),
                    PROCESS_SECURITY_ENVIRONMENT_FLAG_NONE,
                )
            })
            .and_then(|environment| {
                let startup_info =
                    SecurityEnvironmentStartupInfo::new(Default::default(), environment.raw(), &[]);
                environment.close();
                startup_info.map(drop)
            })
            .is_ok()
    })
}
