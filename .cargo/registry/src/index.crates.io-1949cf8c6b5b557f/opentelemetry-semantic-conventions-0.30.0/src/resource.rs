// DO NOT EDIT, this is an auto-generated file
//
// If you want to update the file:
// - Edit the template at scripts/templates/registry/rust/resource.rs.j2
// - Run the script at scripts/generate-consts-from-spec.sh

//! # Resource Semantic Conventions
//!
//! The [resource semantic conventions] define a set of standardized attributes
//! to be used in `Resource`s.
//!
//! [resource semantic conventions]: https://opentelemetry.io/docs/specs/semconv/resource/
//!
//! ## Usage
//!
//! ```rust
//! use opentelemetry::KeyValue;
//! use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
//! use opentelemetry_semantic_conventions as semconv;
//!
//! let _tracer = SdkTracerProvider::builder()
//!     .with_resource(Resource::builder_empty().with_service_name("my-service").build())
//!     .build();
//! ```

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::ANDROID_OS_API_LEVEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::APP_INSTALLATION_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_ECS_CLUSTER_ARN;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_ECS_CONTAINER_ARN;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_ECS_LAUNCHTYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_ECS_TASK_ARN;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_ECS_TASK_FAMILY;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_ECS_TASK_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_ECS_TASK_REVISION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_EKS_CLUSTER_ARN;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_LOG_GROUP_ARNS;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_LOG_GROUP_NAMES;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_LOG_STREAM_ARNS;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::AWS_LOG_STREAM_NAMES;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::BROWSER_BRANDS;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::BROWSER_LANGUAGE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::BROWSER_MOBILE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::BROWSER_PLATFORM;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CICD_PIPELINE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CICD_PIPELINE_RUN_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CICD_PIPELINE_RUN_URL_FULL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CICD_WORKER_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CICD_WORKER_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CICD_WORKER_URL_FULL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUD_ACCOUNT_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUD_AVAILABILITY_ZONE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUD_PLATFORM;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUD_PROVIDER;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUD_REGION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUD_RESOURCE_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_APP_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_APP_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_ORG_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_ORG_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_PROCESS_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_PROCESS_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_SPACE_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_SPACE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_SYSTEM_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CLOUDFOUNDRY_SYSTEM_INSTANCE_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_COMMAND;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_COMMAND_ARGS;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_COMMAND_LINE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_IMAGE_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_IMAGE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_IMAGE_REPO_DIGESTS;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_IMAGE_TAGS;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::CONTAINER_RUNTIME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::DEPLOYMENT_ENVIRONMENT_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::DEVICE_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::DEVICE_MANUFACTURER;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::DEVICE_MODEL_IDENTIFIER;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::DEVICE_MODEL_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::FAAS_INSTANCE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::FAAS_MAX_MEMORY;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::FAAS_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::FAAS_VERSION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_APPLICATION_CONTAINER;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_APPLICATION_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_APPLICATION_LOCATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_SERVICE_CRITICALITY_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_SERVICE_ENVIRONMENT_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_SERVICE_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_WORKLOAD_CRITICALITY_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_WORKLOAD_ENVIRONMENT_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_APPHUB_WORKLOAD_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_CLOUD_RUN_JOB_EXECUTION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_CLOUD_RUN_JOB_TASK_INDEX;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_GCE_INSTANCE_HOSTNAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::GCP_GCE_INSTANCE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HEROKU_APP_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HEROKU_RELEASE_COMMIT;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HEROKU_RELEASE_CREATION_TIMESTAMP;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_ARCH;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_CPU_CACHE_L2_SIZE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_CPU_FAMILY;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_CPU_MODEL_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_CPU_MODEL_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_CPU_STEPPING;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_CPU_VENDOR_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_IMAGE_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_IMAGE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_IMAGE_VERSION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_IP;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_MAC;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::HOST_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CLUSTER_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CLUSTER_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CONTAINER_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CONTAINER_RESTART_COUNT;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CONTAINER_STATUS_LAST_TERMINATED_REASON;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CRONJOB_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CRONJOB_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CRONJOB_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_CRONJOB_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DAEMONSET_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DAEMONSET_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DAEMONSET_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DAEMONSET_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DEPLOYMENT_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DEPLOYMENT_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DEPLOYMENT_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_DEPLOYMENT_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_HPA_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_HPA_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_JOB_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_JOB_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_JOB_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_JOB_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_NAMESPACE_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_NAMESPACE_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_NAMESPACE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_NODE_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_NODE_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_NODE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_NODE_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_POD_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_POD_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_POD_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_POD_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_REPLICASET_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_REPLICASET_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_REPLICASET_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_REPLICASET_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_REPLICATIONCONTROLLER_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_REPLICATIONCONTROLLER_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_RESOURCEQUOTA_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_RESOURCEQUOTA_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_STATEFULSET_ANNOTATION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_STATEFULSET_LABEL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_STATEFULSET_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::K8S_STATEFULSET_UID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::OCI_MANIFEST_DIGEST;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::OS_BUILD_ID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::OS_DESCRIPTION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::OS_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::OS_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::OS_VERSION;

pub use crate::attribute::OTEL_SCOPE_NAME;

pub use crate::attribute::OTEL_SCOPE_VERSION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_COMMAND;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_COMMAND_ARGS;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_COMMAND_LINE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_EXECUTABLE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_EXECUTABLE_PATH;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_LINUX_CGROUP;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_OWNER;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_PARENT_PID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_PID;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_RUNTIME_DESCRIPTION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_RUNTIME_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::PROCESS_RUNTIME_VERSION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::SERVICE_INSTANCE_ID;

pub use crate::attribute::SERVICE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::SERVICE_NAMESPACE;

pub use crate::attribute::SERVICE_VERSION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::TELEMETRY_DISTRO_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::TELEMETRY_DISTRO_VERSION;

pub use crate::attribute::TELEMETRY_SDK_LANGUAGE;

pub use crate::attribute::TELEMETRY_SDK_NAME;

pub use crate::attribute::TELEMETRY_SDK_VERSION;

pub use crate::attribute::USER_AGENT_ORIGINAL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::VCS_REF_HEAD_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::VCS_REF_HEAD_REVISION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::VCS_REF_TYPE;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::VCS_REPOSITORY_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::VCS_REPOSITORY_URL_FULL;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::WEBENGINE_DESCRIPTION;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::WEBENGINE_NAME;

#[cfg(feature = "semconv_experimental")]
pub use crate::attribute::WEBENGINE_VERSION;
