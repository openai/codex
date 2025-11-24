use std::{collections::BTreeMap, marker::PhantomData};

use paste::paste;
use serde::{Deserialize, Serialize};

use super::JsonObject;
pub type ExperimentalCapabilities = BTreeMap<String, JsonObject>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PromptsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ResourcesCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ToolsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RootsCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Capability for handling elicitation requests from servers.
///
/// Elicitation allows servers to request interactive input from users during tool execution.
/// This capability indicates that a client can handle elicitation requests and present
/// appropriate UI to users for collecting the requested information.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ElicitationCapability {
    /// Whether the client supports JSON Schema validation for elicitation responses.
    /// When true, the client will validate user input against the requested_schema
    /// before sending the response back to the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_validation: Option<bool>,
}

///
/// # Builder
/// ```rust
/// # use rmcp::model::ClientCapabilities;
/// let cap = ClientCapabilities::builder()
///     .enable_experimental()
///     .enable_roots()
///     .enable_roots_list_changed()
///     .build();
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<JsonObject>,
    /// Capability to handle elicitation requests from servers for interactive user input
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<ElicitationCapability>,
}

///
/// ## Builder
/// ```rust
/// # use rmcp::model::ServerCapabilities;
/// let cap = ServerCapabilities::builder()
///     .enable_logging()
///     .enable_experimental()
///     .enable_prompts()
///     .enable_resources()
///     .enable_tools()
///     .enable_tool_list_changed()
///     .build();
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<JsonObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<JsonObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

macro_rules! builder {
    ($Target: ident {$($f: ident: $T: ty),* $(,)?}) => {
        paste! {
            #[derive(Default, Clone, Copy, Debug)]
            pub struct [<$Target BuilderState>]<
                $(const [<$f:upper>]: bool = false,)*
            >;
            #[derive(Debug, Default)]
            pub struct [<$Target Builder>]<S = [<$Target BuilderState>]> {
                $(pub $f: Option<$T>,)*
                pub state: PhantomData<S>
            }
            impl $Target {
                #[doc = "Create a new [`" $Target "`] builder."]
                pub fn builder() -> [<$Target Builder>] {
                    <[<$Target Builder>]>::default()
                }
            }
            impl<S> [<$Target Builder>]<S> {
                pub fn build(self) -> $Target {
                    $Target {
                        $( $f: self.$f, )*
                    }
                }
            }
            impl<S> From<[<$Target Builder>]<S>> for $Target {
                fn from(builder: [<$Target Builder>]<S>) -> Self {
                    builder.build()
                }
            }
        }
        builder!($Target @toggle $($f: $T,) *);

    };
    ($Target: ident @toggle $f0: ident: $T0: ty, $($f: ident: $T: ty,)*) => {
        builder!($Target @toggle [][$f0: $T0][$($f: $T,)*]);
    };
    ($Target: ident @toggle [$($ff: ident: $Tf: ty,)*][$fn: ident: $TN: ty][$fn_1: ident: $Tn_1: ty, $($ft: ident: $Tt: ty,)*]) => {
        builder!($Target @impl_toggle [$($ff: $Tf,)*][$fn: $TN][$fn_1: $Tn_1, $($ft:$Tt,)*]);
        builder!($Target @toggle [$($ff: $Tf,)* $fn: $TN,][$fn_1: $Tn_1][$($ft:$Tt,)*]);
    };
    ($Target: ident @toggle [$($ff: ident: $Tf: ty,)*][$fn: ident: $TN: ty][]) => {
        builder!($Target @impl_toggle [$($ff: $Tf,)*][$fn: $TN][]);
    };
    ($Target: ident @impl_toggle [$($ff: ident: $Tf: ty,)*][$fn: ident: $TN: ty][$($ft: ident: $Tt: ty,)*]) => {
        paste! {
            impl<
                $(const [<$ff:upper>]: bool,)*
                $(const [<$ft:upper>]: bool,)*
            > [<$Target Builder>]<[<$Target BuilderState>]<
                $([<$ff:upper>],)*
                false,
                $([<$ft:upper>],)*
            >> {
                pub fn [<enable_ $fn>](self) -> [<$Target Builder>]<[<$Target BuilderState>]<
                    $([<$ff:upper>],)*
                    true,
                    $([<$ft:upper>],)*
                >> {
                    [<$Target Builder>] {
                        $( $ff: self.$ff, )*
                        $fn: Some($TN::default()),
                        $( $ft: self.$ft, )*
                        state: PhantomData
                    }
                }
                pub fn [<enable_ $fn _with>](self, $fn: $TN) -> [<$Target Builder>]<[<$Target BuilderState>]<
                    $([<$ff:upper>],)*
                    true,
                    $([<$ft:upper>],)*
                >> {
                    [<$Target Builder>] {
                        $( $ff: self.$ff, )*
                        $fn: Some($fn),
                        $( $ft: self.$ft, )*
                        state: PhantomData
                    }
                }
            }
            // do we really need to disable some thing in builder?
            // impl<
            //     $(const [<$ff:upper>]: bool,)*
            //     $(const [<$ft:upper>]: bool,)*
            // > [<$Target Builder>]<[<$Target BuilderState>]<
            //     $([<$ff:upper>],)*
            //     true,
            //     $([<$ft:upper>],)*
            // >> {
            //     pub fn [<disable_ $fn>](self) -> [<$Target Builder>]<[<$Target BuilderState>]<
            //         $([<$ff:upper>],)*
            //         false,
            //         $([<$ft:upper>],)*
            //     >> {
            //         [<$Target Builder>] {
            //             $( $ff: self.$ff, )*
            //             $fn: None,
            //             $( $ft: self.$ft, )*
            //             state: PhantomData
            //         }
            //     }
            // }
        }
    }
}

builder! {
    ServerCapabilities {
        experimental: ExperimentalCapabilities,
        logging: JsonObject,
        completions: JsonObject,
        prompts: PromptsCapability,
        resources: ResourcesCapability,
        tools: ToolsCapability
    }
}

impl<const E: bool, const L: bool, const C: bool, const P: bool, const R: bool>
    ServerCapabilitiesBuilder<ServerCapabilitiesBuilderState<E, L, C, P, R, true>>
{
    pub fn enable_tool_list_changed(mut self) -> Self {
        if let Some(c) = self.tools.as_mut() {
            c.list_changed = Some(true);
        }
        self
    }
}

impl<const E: bool, const L: bool, const C: bool, const R: bool, const T: bool>
    ServerCapabilitiesBuilder<ServerCapabilitiesBuilderState<E, L, C, true, R, T>>
{
    pub fn enable_prompts_list_changed(mut self) -> Self {
        if let Some(c) = self.prompts.as_mut() {
            c.list_changed = Some(true);
        }
        self
    }
}

impl<const E: bool, const L: bool, const C: bool, const P: bool, const T: bool>
    ServerCapabilitiesBuilder<ServerCapabilitiesBuilderState<E, L, C, P, true, T>>
{
    pub fn enable_resources_list_changed(mut self) -> Self {
        if let Some(c) = self.resources.as_mut() {
            c.list_changed = Some(true);
        }
        self
    }

    pub fn enable_resources_subscribe(mut self) -> Self {
        if let Some(c) = self.resources.as_mut() {
            c.subscribe = Some(true);
        }
        self
    }
}

builder! {
    ClientCapabilities{
        experimental: ExperimentalCapabilities,
        roots: RootsCapabilities,
        sampling: JsonObject,
        elicitation: ElicitationCapability,
    }
}

impl<const E: bool, const S: bool>
    ClientCapabilitiesBuilder<ClientCapabilitiesBuilderState<E, true, S>>
{
    pub fn enable_roots_list_changed(mut self) -> Self {
        if let Some(c) = self.roots.as_mut() {
            c.list_changed = Some(true);
        }
        self
    }
}

#[cfg(feature = "elicitation")]
impl<const E: bool, const R: bool, const S: bool>
    ClientCapabilitiesBuilder<ClientCapabilitiesBuilderState<E, R, S, true>>
{
    /// Enable JSON Schema validation for elicitation responses.
    /// When enabled, the client will validate user input against the requested_schema
    /// before sending responses back to the server.
    pub fn enable_elicitation_schema_validation(mut self) -> Self {
        if let Some(c) = self.elicitation.as_mut() {
            c.schema_validation = Some(true);
        }
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_builder() {
        let builder = <ServerCapabilitiesBuilder>::default()
            .enable_logging()
            .enable_experimental()
            .enable_prompts()
            .enable_resources()
            .enable_tools()
            .enable_tool_list_changed();
        assert_eq!(builder.logging, Some(JsonObject::default()));
        assert_eq!(builder.prompts, Some(PromptsCapability::default()));
        assert_eq!(builder.resources, Some(ResourcesCapability::default()));
        assert_eq!(
            builder.tools,
            Some(ToolsCapability {
                list_changed: Some(true),
            })
        );
        assert_eq!(
            builder.experimental,
            Some(ExperimentalCapabilities::default())
        );
        let client_builder = <ClientCapabilitiesBuilder>::default()
            .enable_experimental()
            .enable_roots()
            .enable_roots_list_changed()
            .enable_sampling();
        assert_eq!(
            client_builder.experimental,
            Some(ExperimentalCapabilities::default())
        );
        assert_eq!(
            client_builder.roots,
            Some(RootsCapabilities {
                list_changed: Some(true),
            })
        );
    }
}
