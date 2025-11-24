use super::*;

use serde::{de::DeserializeOwned, Serialize};

pub trait Request {
    type Params: DeserializeOwned + Serialize;
    type Result: DeserializeOwned + Serialize;
    const METHOD: &'static str;
}

#[macro_export]
macro_rules! lsp_request {
    ("initialize") => {
        $crate::request::Initialize
    };
    ("shutdown") => {
        $crate::request::Shutdown
    };

    ("window/showMessageRequest") => {
        $crate::request::ShowMessageRequest
    };

    ("client/registerCapability") => {
        $crate::request::RegisterCapability
    };
    ("client/unregisterCapability") => {
        $crate::request::UnregisterCapability
    };

    ("workspace/symbol") => {
        $crate::request::WorkspaceSymbolRequest
    };
    ("workspaceSymbol/resolve") => {
        $crate::request::WorkspaceSymbolResolve
    };
    ("workspace/executeCommand") => {
        $crate::request::ExecuteCommand
    };

    ("textDocument/willSaveWaitUntil") => {
        $crate::request::WillSaveWaitUntil
    };

    ("textDocument/completion") => {
        $crate::request::Completion
    };
    ("completionItem/resolve") => {
        $crate::request::ResolveCompletionItem
    };
    ("textDocument/hover") => {
        $crate::request::HoverRequest
    };
    ("textDocument/signatureHelp") => {
        $crate::request::SignatureHelpRequest
    };
    ("textDocument/declaration") => {
        $crate::request::GotoDeclaration
    };
    ("textDocument/definition") => {
        $crate::request::GotoDefinition
    };
    ("textDocument/references") => {
        $crate::request::References
    };
    ("textDocument/documentHighlight") => {
        $crate::request::DocumentHighlightRequest
    };
    ("textDocument/documentSymbol") => {
        $crate::request::DocumentSymbolRequest
    };
    ("textDocument/codeAction") => {
        $crate::request::CodeActionRequest
    };
    ("textDocument/codeLens") => {
        $crate::request::CodeLensRequest
    };
    ("codeLens/resolve") => {
        $crate::request::CodeLensResolve
    };
    ("textDocument/documentLink") => {
        $crate::request::DocumentLinkRequest
    };
    ("documentLink/resolve") => {
        $crate::request::DocumentLinkResolve
    };
    ("workspace/applyEdit") => {
        $crate::request::ApplyWorkspaceEdit
    };
    ("textDocument/rangeFormatting") => {
        $crate::request::RangeFormatting
    };
    ("textDocument/onTypeFormatting") => {
        $crate::request::OnTypeFormatting
    };
    ("textDocument/formatting") => {
        $crate::request::Formatting
    };
    ("textDocument/rename") => {
        $crate::request::Rename
    };
    ("textDocument/documentColor") => {
        $crate::request::DocumentColor
    };
    ("textDocument/colorPresentation") => {
        $crate::request::ColorPresentationRequest
    };
    ("textDocument/foldingRange") => {
        $crate::request::FoldingRangeRequest
    };
    ("textDocument/prepareRename") => {
        $crate::request::PrepareRenameRequest
    };
    ("textDocument/implementation") => {
        $crate::request::GotoImplementation
    };
    ("textDocument/typeDefinition") => {
        $crate::request::GotoTypeDefinition
    };
    ("textDocument/selectionRange") => {
        $crate::request::SelectionRangeRequest
    };
    ("workspace/workspaceFolders") => {
        $crate::request::WorkspaceFoldersRequest
    };
    ("workspace/configuration") => {
        $crate::request::WorkspaceConfiguration
    };
    ("window/workDoneProgress/create") => {
        $crate::request::WorkDoneProgressCreate
    };
    ("callHierarchy/incomingCalls") => {
        $crate::request::CallHierarchyIncomingCalls
    };
    ("callHierarchy/outgoingCalls") => {
        $crate::request::CallHierarchyOutgoingCalls
    };
    ("textDocument/moniker") => {
        $crate::request::MonikerRequest
    };
    ("textDocument/linkedEditingRange") => {
        $crate::request::LinkedEditingRange
    };
    ("textDocument/prepareCallHierarchy") => {
        $crate::request::CallHierarchyPrepare
    };
    ("textDocument/prepareTypeHierarchy") => {
        $crate::request::TypeHierarchyPrepare
    };
    ("textDocument/semanticTokens/full") => {
        $crate::request::SemanticTokensFullRequest
    };
    ("textDocument/semanticTokens/full/delta") => {
        $crate::request::SemanticTokensFullDeltaRequest
    };
    ("textDocument/semanticTokens/range") => {
        $crate::request::SemanticTokensRangeRequest
    };
    ("textDocument/inlayHint") => {
        $crate::request::InlayHintRequest
    };
    ("textDocument/inlineValue") => {
        $crate::request::InlineValueRequest
    };
    ("textDocument/diagnostic") => {
        $crate::request::DocumentDiagnosticRequest
    };
    ("workspace/diagnostic") => {
        $crate::request::WorkspaceDiagnosticRequest
    };
    ("workspace/diagnostic/refresh") => {
        $crate::request::WorkspaceDiagnosticRefresh
    };
    ("typeHierarchy/supertypes") => {
        $crate::request::TypeHierarchySupertypes
    };
    ("typeHierarchy/subtypes") => {
        $crate::request::TypeHierarchySubtypes
    };
    ("workspace/willCreateFiles") => {
        $crate::request::WillCreateFiles
    };
    ("workspace/willRenameFiles") => {
        $crate::request::WillRenameFiles
    };
    ("workspace/willDeleteFiles") => {
        $crate::request::WillDeleteFiles
    };
    ("workspace/semanticTokens/refresh") => {
        $crate::request::SemanticTokensRefresh
    };
    ("workspace/codeLens/refresh") => {
        $crate::request::CodeLensRefresh
    };
    ("workspace/inlayHint/refresh") => {
        $crate::request::InlayHintRefreshRequest
    };
    ("workspace/inlineValue/refresh") => {
        $crate::request::InlineValueRefreshRequest
    };
    ("codeAction/resolve") => {
        $crate::request::CodeActionResolveRequest
    };
    ("inlayHint/resolve") => {
        $crate::request::InlayHintResolveRequest
    };
    ("window/showDocument") => {
        $crate::request::ShowDocument
    };
}

/// The initialize request is sent as the first request from the client to the server.
/// If the server receives request or notification before the `initialize` request it should act as follows:
///
/// * for a request the respond should be errored with `code: -32001`. The message can be picked by the server.
/// * notifications should be dropped.
#[derive(Debug)]
pub enum Initialize {}

impl Request for Initialize {
    type Params = InitializeParams;
    type Result = InitializeResult;
    const METHOD: &'static str = "initialize";
}

/// The shutdown request is sent from the client to the server. It asks the server to shut down,
/// but to not exit (otherwise the response might not be delivered correctly to the client).
/// There is a separate exit notification that asks the server to exit.
#[derive(Debug)]
pub enum Shutdown {}

impl Request for Shutdown {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "shutdown";
}

/// The show message request is sent from a server to a client to ask the client to display a particular message
/// in the user interface. In addition to the show message notification the request allows to pass actions and to
/// wait for an answer from the client.
#[derive(Debug)]
pub enum ShowMessageRequest {}

impl Request for ShowMessageRequest {
    type Params = ShowMessageRequestParams;
    type Result = Option<MessageActionItem>;
    const METHOD: &'static str = "window/showMessageRequest";
}

/// The client/registerCapability request is sent from the server to the client to register for a new capability
/// on the client side. Not all clients need to support dynamic capability registration. A client opts in via the
/// ClientCapabilities.GenericCapability property.
#[derive(Debug)]
pub enum RegisterCapability {}

impl Request for RegisterCapability {
    type Params = RegistrationParams;
    type Result = ();
    const METHOD: &'static str = "client/registerCapability";
}

/// The client/unregisterCapability request is sent from the server to the client to unregister a
/// previously register capability.
#[derive(Debug)]
pub enum UnregisterCapability {}

impl Request for UnregisterCapability {
    type Params = UnregistrationParams;
    type Result = ();
    const METHOD: &'static str = "client/unregisterCapability";
}

/// The Completion request is sent from the client to the server to compute completion items at a given cursor position.
/// Completion items are presented in the IntelliSense user interface. If computing full completion items is expensive,
/// servers can additionally provide a handler for the completion item resolve request ('completionItem/resolve').
/// This request is sent when a completion item is selected in the user interface. A typical use case is for example:
/// the 'textDocument/completion' request doesn’t fill in the documentation property for returned completion items
/// since it is expensive to compute. When the item is selected in the user interface then a ‘completionItem/resolve’
/// request is sent with the selected completion item as a param. The returned completion item should have the
/// documentation property filled in. The request can delay the computation of the detail and documentation properties.
/// However, properties that are needed for the initial sorting and filtering, like sortText, filterText, insertText,
/// and textEdit must be provided in the textDocument/completion request and must not be changed during resolve.
#[derive(Debug)]
pub enum Completion {}

impl Request for Completion {
    type Params = CompletionParams;
    type Result = Option<CompletionResponse>;
    const METHOD: &'static str = "textDocument/completion";
}

/// The request is sent from the client to the server to resolve additional information for a given completion item.
#[derive(Debug)]
pub enum ResolveCompletionItem {}

impl Request for ResolveCompletionItem {
    type Params = CompletionItem;
    type Result = CompletionItem;
    const METHOD: &'static str = "completionItem/resolve";
}

/// The hover request is sent from the client to the server to request hover information at a given text
/// document position.
#[derive(Debug)]
pub enum HoverRequest {}

impl Request for HoverRequest {
    type Params = HoverParams;
    type Result = Option<Hover>;
    const METHOD: &'static str = "textDocument/hover";
}

/// The signature help request is sent from the client to the server to request signature information at
/// a given cursor position.
#[derive(Debug)]
pub enum SignatureHelpRequest {}

impl Request for SignatureHelpRequest {
    type Params = SignatureHelpParams;
    type Result = Option<SignatureHelp>;
    const METHOD: &'static str = "textDocument/signatureHelp";
}

#[derive(Debug)]
pub enum GotoDeclaration {}
pub type GotoDeclarationParams = GotoDefinitionParams;
pub type GotoDeclarationResponse = GotoDefinitionResponse;

/// The goto declaration request is sent from the client to the server to resolve the declaration location of
/// a symbol at a given text document position.
impl Request for GotoDeclaration {
    type Params = GotoDeclarationParams;
    type Result = Option<GotoDeclarationResponse>;
    const METHOD: &'static str = "textDocument/declaration";
}

/// The goto definition request is sent from the client to the server to resolve the definition location of
/// a symbol at a given text document position.
#[derive(Debug)]
pub enum GotoDefinition {}

impl Request for GotoDefinition {
    type Params = GotoDefinitionParams;
    type Result = Option<GotoDefinitionResponse>;
    const METHOD: &'static str = "textDocument/definition";
}

/// The references request is sent from the client to the server to resolve project-wide references for the
/// symbol denoted by the given text document position.
#[derive(Debug)]
pub enum References {}

impl Request for References {
    type Params = ReferenceParams;
    type Result = Option<Vec<Location>>;
    const METHOD: &'static str = "textDocument/references";
}

/// The goto type definition request is sent from the client to the
/// server to resolve the type definition location of a symbol at a
/// given text document position.
#[derive(Debug)]
pub enum GotoTypeDefinition {}

pub type GotoTypeDefinitionParams = GotoDefinitionParams;
pub type GotoTypeDefinitionResponse = GotoDefinitionResponse;

impl Request for GotoTypeDefinition {
    type Params = GotoTypeDefinitionParams;
    type Result = Option<GotoTypeDefinitionResponse>;
    const METHOD: &'static str = "textDocument/typeDefinition";
}

/// The goto implementation request is sent from the client to the
/// server to resolve the implementation location of a symbol at a
/// given text document position.
#[derive(Debug)]
pub enum GotoImplementation {}

pub type GotoImplementationParams = GotoTypeDefinitionParams;
pub type GotoImplementationResponse = GotoDefinitionResponse;

impl Request for GotoImplementation {
    type Params = GotoImplementationParams;
    type Result = Option<GotoImplementationResponse>;
    const METHOD: &'static str = "textDocument/implementation";
}

/// The document highlight request is sent from the client to the server to resolve a document highlights
/// for a given text document position.
/// For programming languages this usually highlights all references to the symbol scoped to this file.
/// However we kept 'textDocument/documentHighlight' and 'textDocument/references' separate requests since
/// the first one is allowed to be more fuzzy.
/// Symbol matches usually have a DocumentHighlightKind of Read or Write whereas fuzzy or textual matches
/// use Text as the kind.
#[derive(Debug)]
pub enum DocumentHighlightRequest {}

impl Request for DocumentHighlightRequest {
    type Params = DocumentHighlightParams;
    type Result = Option<Vec<DocumentHighlight>>;
    const METHOD: &'static str = "textDocument/documentHighlight";
}

/// The document symbol request is sent from the client to the server to list all symbols found in a given
/// text document.
#[derive(Debug)]
pub enum DocumentSymbolRequest {}

impl Request for DocumentSymbolRequest {
    type Params = DocumentSymbolParams;
    type Result = Option<DocumentSymbolResponse>;
    const METHOD: &'static str = "textDocument/documentSymbol";
}

/// The workspace symbol request is sent from the client to the server to list project-wide symbols
/// matching the query string.
#[derive(Debug)]
pub enum WorkspaceSymbolRequest {}

impl Request for WorkspaceSymbolRequest {
    type Params = WorkspaceSymbolParams;
    type Result = Option<WorkspaceSymbolResponse>;
    const METHOD: &'static str = "workspace/symbol";
}

/// The `workspaceSymbol/resolve` request is sent from the client to the server to resolve
/// additional information for a given workspace symbol.
#[derive(Debug)]
pub enum WorkspaceSymbolResolve {}

impl Request for WorkspaceSymbolResolve {
    type Params = WorkspaceSymbol;
    type Result = WorkspaceSymbol;
    const METHOD: &'static str = "workspaceSymbol/resolve";
}

/// The workspace/executeCommand request is sent from the client to the server to trigger command execution on the server.
/// In most cases the server creates a WorkspaceEdit structure and applies the changes to the workspace using the request
/// workspace/applyEdit which is sent from the server to the client.
#[derive(Debug)]
pub enum ExecuteCommand {}

impl Request for ExecuteCommand {
    type Params = ExecuteCommandParams;
    type Result = Option<Value>;
    const METHOD: &'static str = "workspace/executeCommand";
}

/// The document will save request is sent from the client to the server before the document is
/// actually saved. The request can return an array of TextEdits which will be applied to the text
/// document before it is saved. Please note that clients might drop results if computing the text
/// edits took too long or if a server constantly fails on this request. This is done to keep the
/// save fast and reliable.
#[derive(Debug)]
pub enum WillSaveWaitUntil {}

impl Request for WillSaveWaitUntil {
    type Params = WillSaveTextDocumentParams;
    type Result = Option<Vec<TextEdit>>;
    const METHOD: &'static str = "textDocument/willSaveWaitUntil";
}

/// The workspace/applyEdit request is sent from the server to the client to modify resource on the
/// client side.
#[derive(Debug)]
pub enum ApplyWorkspaceEdit {}

impl Request for ApplyWorkspaceEdit {
    type Params = ApplyWorkspaceEditParams;
    type Result = ApplyWorkspaceEditResponse;
    const METHOD: &'static str = "workspace/applyEdit";
}

/// The workspace/configuration request is sent from the server to the client to fetch configuration settings
/// from the client. The request can fetch several configuration settings in one roundtrip.
/// The order of the returned configuration settings correspond to the order of the passed ConfigurationItems
/// (e.g. the first item in the response is the result for the first configuration item in the params).
///
/// A ConfigurationItem consists of the configuration section to ask for and an additional scope URI.
/// The configuration section ask for is defined by the server and doesn’t necessarily need to correspond to
/// the configuration store used be the client. So a server might ask for a configuration cpp.formatterOptions
/// but the client stores the configuration in a XML store layout differently.
/// It is up to the client to do the necessary conversion. If a scope URI is provided the client should return
/// the setting scoped to the provided resource. If the client for example uses EditorConfig to manage its
/// settings the configuration should be returned for the passed resource URI. If the client can’t provide a
/// configuration setting for a given scope then null need to be present in the returned array.
#[derive(Debug)]
pub enum WorkspaceConfiguration {}

impl Request for WorkspaceConfiguration {
    type Params = ConfigurationParams;
    type Result = Vec<Value>;
    const METHOD: &'static str = "workspace/configuration";
}

/// The code action request is sent from the client to the server to compute commands for a given text document
/// and range. The request is triggered when the user moves the cursor into a problem marker in the editor or
/// presses the lightbulb associated with a marker.
#[derive(Debug)]
pub enum CodeActionRequest {}

impl Request for CodeActionRequest {
    type Params = CodeActionParams;
    type Result = Option<CodeActionResponse>;
    const METHOD: &'static str = "textDocument/codeAction";
}

/// The request is sent from the client to the server to resolve additional information for a given code action.
/// This is usually used to compute the `edit` property of a code action to avoid its unnecessary computation
/// during the `textDocument/codeAction` request.
///
/// @since 3.16.0
#[derive(Debug)]
pub enum CodeActionResolveRequest {}

impl Request for CodeActionResolveRequest {
    type Params = CodeAction;
    type Result = CodeAction;
    const METHOD: &'static str = "codeAction/resolve";
}

/// The code lens request is sent from the client to the server to compute code lenses for a given text document.
#[derive(Debug)]
pub enum CodeLensRequest {}

impl Request for CodeLensRequest {
    type Params = CodeLensParams;
    type Result = Option<Vec<CodeLens>>;
    const METHOD: &'static str = "textDocument/codeLens";
}

/// The code lens resolve request is sent from the client to the server to resolve the command for a
/// given code lens item.
#[derive(Debug)]
pub enum CodeLensResolve {}

impl Request for CodeLensResolve {
    type Params = CodeLens;
    type Result = CodeLens;
    const METHOD: &'static str = "codeLens/resolve";
}

/// The document links request is sent from the client to the server to request the location of links in a document.
#[derive(Debug)]
pub enum DocumentLinkRequest {}

impl Request for DocumentLinkRequest {
    type Params = DocumentLinkParams;
    type Result = Option<Vec<DocumentLink>>;
    const METHOD: &'static str = "textDocument/documentLink";
}

/// The document link resolve request is sent from the client to the server to resolve the target of
/// a given document link.
#[derive(Debug)]
pub enum DocumentLinkResolve {}

impl Request for DocumentLinkResolve {
    type Params = DocumentLink;
    type Result = DocumentLink;
    const METHOD: &'static str = "documentLink/resolve";
}

/// The document formatting request is sent from the server to the client to format a whole document.
#[derive(Debug)]
pub enum Formatting {}

impl Request for Formatting {
    type Params = DocumentFormattingParams;
    type Result = Option<Vec<TextEdit>>;
    const METHOD: &'static str = "textDocument/formatting";
}

/// The document range formatting request is sent from the client to the server to format a given range in a document.
#[derive(Debug)]
pub enum RangeFormatting {}

impl Request for RangeFormatting {
    type Params = DocumentRangeFormattingParams;
    type Result = Option<Vec<TextEdit>>;
    const METHOD: &'static str = "textDocument/rangeFormatting";
}

/// The document on type formatting request is sent from the client to the server to format parts of
/// the document during typing.
#[derive(Debug)]
pub enum OnTypeFormatting {}

impl Request for OnTypeFormatting {
    type Params = DocumentOnTypeFormattingParams;
    type Result = Option<Vec<TextEdit>>;
    const METHOD: &'static str = "textDocument/onTypeFormatting";
}

/// The linked editing request is sent from the client to the server to return for a given position in a document
/// the range of the symbol at the position and all ranges that have the same content.
/// Optionally a word pattern can be returned to describe valid contents. A rename to one of the ranges can be applied
/// to all other ranges if the new content is valid. If no result-specific word pattern is provided, the word pattern from
/// the client’s language configuration is used.
#[derive(Debug)]
pub enum LinkedEditingRange {}

impl Request for LinkedEditingRange {
    type Params = LinkedEditingRangeParams;
    type Result = Option<LinkedEditingRanges>;
    const METHOD: &'static str = "textDocument/linkedEditingRange";
}

/// The rename request is sent from the client to the server to perform a workspace-wide rename of a symbol.
#[derive(Debug)]
pub enum Rename {}

impl Request for Rename {
    type Params = RenameParams;
    type Result = Option<WorkspaceEdit>;
    const METHOD: &'static str = "textDocument/rename";
}

/// The document color request is sent from the client to the server to list all color references found in a given text document.
/// Along with the range, a color value in RGB is returned.
#[derive(Debug)]
pub enum DocumentColor {}

impl Request for DocumentColor {
    type Params = DocumentColorParams;
    type Result = Vec<ColorInformation>;
    const METHOD: &'static str = "textDocument/documentColor";
}

/// The color presentation request is sent from the client to the server to obtain a list of presentations for a color value
/// at a given location.
#[derive(Debug)]
pub enum ColorPresentationRequest {}

impl Request for ColorPresentationRequest {
    type Params = ColorPresentationParams;
    type Result = Vec<ColorPresentation>;
    const METHOD: &'static str = "textDocument/colorPresentation";
}

/// The folding range request is sent from the client to the server to return all folding ranges found in a given text document.
#[derive(Debug)]
pub enum FoldingRangeRequest {}

impl Request for FoldingRangeRequest {
    type Params = FoldingRangeParams;
    type Result = Option<Vec<FoldingRange>>;
    const METHOD: &'static str = "textDocument/foldingRange";
}

/// The prepare rename request is sent from the client to the server to setup and test the validity of a rename operation
/// at a given location.
#[derive(Debug)]
pub enum PrepareRenameRequest {}

impl Request for PrepareRenameRequest {
    type Params = TextDocumentPositionParams;
    type Result = Option<PrepareRenameResponse>;
    const METHOD: &'static str = "textDocument/prepareRename";
}

/// The workspace/workspaceFolders request is sent from the server to the client to fetch the current open list of
/// workspace folders. Returns null in the response if only a single file is open in the tool.
/// Returns an empty array if a workspace is open but no folders are configured.
#[derive(Debug)]
pub enum WorkspaceFoldersRequest {}

impl Request for WorkspaceFoldersRequest {
    type Params = ();
    type Result = Option<Vec<WorkspaceFolder>>;
    const METHOD: &'static str = "workspace/workspaceFolders";
}

/// The `window/workDoneProgress/create` request is sent from the server
/// to the client to ask the client to create a work done progress.
#[derive(Debug)]
pub enum WorkDoneProgressCreate {}

impl Request for WorkDoneProgressCreate {
    type Params = WorkDoneProgressCreateParams;
    type Result = ();
    const METHOD: &'static str = "window/workDoneProgress/create";
}

/// The selection range request is sent from the client to the server to return
/// suggested selection ranges at given positions. A selection range is a range
/// around the cursor position which the user might be interested in selecting.
///
/// A selection range in the return array is for the position in the provided parameters at the same index.
/// Therefore `positions[i]` must be contained in `result[i].range`.
///
/// Typically, but not necessary, selection ranges correspond to the nodes of the
/// syntax tree.
pub enum SelectionRangeRequest {}

impl Request for SelectionRangeRequest {
    type Params = SelectionRangeParams;
    type Result = Option<Vec<SelectionRange>>;
    const METHOD: &'static str = "textDocument/selectionRange";
}

pub enum CallHierarchyPrepare {}

impl Request for CallHierarchyPrepare {
    type Params = CallHierarchyPrepareParams;
    type Result = Option<Vec<CallHierarchyItem>>;
    const METHOD: &'static str = "textDocument/prepareCallHierarchy";
}

pub enum CallHierarchyIncomingCalls {}

impl Request for CallHierarchyIncomingCalls {
    type Params = CallHierarchyIncomingCallsParams;
    type Result = Option<Vec<CallHierarchyIncomingCall>>;
    const METHOD: &'static str = "callHierarchy/incomingCalls";
}

pub enum CallHierarchyOutgoingCalls {}

impl Request for CallHierarchyOutgoingCalls {
    type Params = CallHierarchyOutgoingCallsParams;
    type Result = Option<Vec<CallHierarchyOutgoingCall>>;
    const METHOD: &'static str = "callHierarchy/outgoingCalls";
}

pub enum SemanticTokensFullRequest {}

impl Request for SemanticTokensFullRequest {
    type Params = SemanticTokensParams;
    type Result = Option<SemanticTokensResult>;
    const METHOD: &'static str = "textDocument/semanticTokens/full";
}

pub enum SemanticTokensFullDeltaRequest {}

impl Request for SemanticTokensFullDeltaRequest {
    type Params = SemanticTokensDeltaParams;
    type Result = Option<SemanticTokensFullDeltaResult>;
    const METHOD: &'static str = "textDocument/semanticTokens/full/delta";
}

pub enum SemanticTokensRangeRequest {}

impl Request for SemanticTokensRangeRequest {
    type Params = SemanticTokensRangeParams;
    type Result = Option<SemanticTokensRangeResult>;
    const METHOD: &'static str = "textDocument/semanticTokens/range";
}

/// The `workspace/semanticTokens/refresh` request is sent from the server to the client.
/// Servers can use it to ask clients to refresh the editors for which this server provides semantic tokens.
/// As a result the client should ask the server to recompute the semantic tokens for these editors.
/// This is useful if a server detects a project wide configuration change which requires a re-calculation of all semantic tokens.
/// Note that the client still has the freedom to delay the re-calculation of the semantic tokens if for example an editor is currently not visible.
pub enum SemanticTokensRefresh {}

impl Request for SemanticTokensRefresh {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "workspace/semanticTokens/refresh";
}

/// The workspace/codeLens/refresh request is sent from the server to the client.
/// Servers can use it to ask clients to refresh the code lenses currently shown in editors.
/// As a result the client should ask the server to recompute the code lenses for these editors.
/// This is useful if a server detects a configuration change which requires a re-calculation of all code lenses.
/// Note that the client still has the freedom to delay the re-calculation of the code lenses if for example an editor is currently not visible.
pub enum CodeLensRefresh {}

impl Request for CodeLensRefresh {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "workspace/codeLens/refresh";
}

/// The will create files request is sent from the client to the server before files are actually created as long as the creation is triggered from within the client. The request can return a WorkspaceEdit which will be applied to workspace before the files are created. Please note that clients might drop results if computing the edit took too long or if a server constantly fails on this request. This is done to keep creates fast and reliable.
pub enum WillCreateFiles {}

impl Request for WillCreateFiles {
    type Params = CreateFilesParams;
    type Result = Option<WorkspaceEdit>;
    const METHOD: &'static str = "workspace/willCreateFiles";
}

/// The will rename files request is sent from the client to the server before files are actually renamed as long as the rename is triggered from within the client. The request can return a WorkspaceEdit which will be applied to workspace before the files are renamed. Please note that clients might drop results if computing the edit took too long or if a server constantly fails on this request. This is done to keep renames fast and reliable.
pub enum WillRenameFiles {}

impl Request for WillRenameFiles {
    type Params = RenameFilesParams;
    type Result = Option<WorkspaceEdit>;
    const METHOD: &'static str = "workspace/willRenameFiles";
}

/// The will delete files request is sent from the client to the server before files are actually deleted as long as the deletion is triggered from within the client. The request can return a WorkspaceEdit which will be applied to workspace before the files are deleted. Please note that clients might drop results if computing the edit took too long or if a server constantly fails on this request. This is done to keep deletes fast and reliable.
pub enum WillDeleteFiles {}

impl Request for WillDeleteFiles {
    type Params = DeleteFilesParams;
    type Result = Option<WorkspaceEdit>;
    const METHOD: &'static str = "workspace/willDeleteFiles";
}

/// The show document request is sent from a server to a client to ask the client to display a particular document in the user interface.
pub enum ShowDocument {}

impl Request for ShowDocument {
    type Params = ShowDocumentParams;
    type Result = ShowDocumentResult;
    const METHOD: &'static str = "window/showDocument";
}

pub enum MonikerRequest {}

impl Request for MonikerRequest {
    type Params = MonikerParams;
    type Result = Option<Vec<Moniker>>;
    const METHOD: &'static str = "textDocument/moniker";
}

/// The inlay hints request is sent from the client to the server to compute inlay hints for a given
/// [text document, range] tuple that may be rendered in the editor in place with other text.
pub enum InlayHintRequest {}

impl Request for InlayHintRequest {
    type Params = InlayHintParams;
    type Result = Option<Vec<InlayHint>>;
    const METHOD: &'static str = "textDocument/inlayHint";
}

/// The `inlayHint/resolve` request is sent from the client to the server to resolve additional
/// information for a given inlay hint. This is usually used to compute the tooltip, location or
/// command properties of a inlay hint’s label part to avoid its unnecessary computation during the
/// `textDocument/inlayHint` request.
pub enum InlayHintResolveRequest {}

impl Request for InlayHintResolveRequest {
    type Params = InlayHint;
    type Result = InlayHint;
    const METHOD: &'static str = "inlayHint/resolve";
}

/// The `workspace/inlayHint/refresh` request is sent from the server to the client. Servers can use
/// it to ask clients to refresh the inlay hints currently shown in editors. As a result the client
/// should ask the server to recompute the inlay hints for these editors. This is useful if a server
/// detects a configuration change which requires a re-calculation of all inlay hints. Note that the
/// client still has the freedom to delay the re-calculation of the inlay hints if for example an
/// editor is currently not visible.
pub enum InlayHintRefreshRequest {}

impl Request for InlayHintRefreshRequest {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "workspace/inlayHint/refresh";
}

/// The inline value request is sent from the client to the server to compute inline values for a
/// given text document that may be rendered in the editor at the end of lines.
pub enum InlineValueRequest {}

impl Request for InlineValueRequest {
    type Params = InlineValueParams;
    type Result = Option<InlineValue>;
    const METHOD: &'static str = "textDocument/inlineValue";
}

/// The `workspace/inlineValue/refresh` request is sent from the server to the client. Servers can
/// use it to ask clients to refresh the inline values currently shown in editors. As a result the
/// client should ask the server to recompute the inline values for these editors. This is useful if
/// a server detects a configuration change which requires a re-calculation of all inline values.
/// Note that the client still has the freedom to delay the re-calculation of the inline values if
/// for example an editor is currently not visible.
pub enum InlineValueRefreshRequest {}

impl Request for InlineValueRefreshRequest {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "workspace/inlineValue/refresh";
}

/// The text document diagnostic request is sent from the client to the server to ask the server to
/// compute the diagnostics for a given document. As with other pull requests the server is asked
/// to compute the diagnostics for the currently synced version of the document.
#[derive(Debug)]
pub enum DocumentDiagnosticRequest {}

impl Request for DocumentDiagnosticRequest {
    type Params = DocumentDiagnosticParams;
    type Result = DocumentDiagnosticReportResult;
    const METHOD: &'static str = "textDocument/diagnostic";
}

/// The workspace diagnostic request is sent from the client to the server to ask the server to
/// compute workspace wide diagnostics which previously where pushed from the server to the client.
/// In contrast to the document diagnostic request the workspace request can be long running and is
/// not bound to a specific workspace or document state. If the client supports streaming for the
/// workspace diagnostic pull it is legal to provide a document diagnostic report multiple times
/// for the same document URI. The last one reported will win over previous reports.
#[derive(Debug)]
pub enum WorkspaceDiagnosticRequest {}

impl Request for WorkspaceDiagnosticRequest {
    type Params = WorkspaceDiagnosticParams;
    const METHOD: &'static str = "workspace/diagnostic";
    type Result = WorkspaceDiagnosticReportResult;
}

/// The `workspace/diagnostic/refresh` request is sent from the server to the client. Servers can
/// use it to ask clients to refresh all needed document and workspace diagnostics. This is useful
/// if a server detects a project wide configuration change which requires a re-calculation of all
/// diagnostics.
#[derive(Debug)]
pub enum WorkspaceDiagnosticRefresh {}

impl Request for WorkspaceDiagnosticRefresh {
    type Params = ();
    type Result = ();
    const METHOD: &'static str = "workspace/diagnostic/refresh";
}

/// The type hierarchy request is sent from the client to the server to return a type hierarchy for
/// the language element of given text document positions. Will return null if the server couldn’t
/// infer a valid type from the position. The type hierarchy requests are executed in two steps:
///
/// 1. first a type hierarchy item is prepared for the given text document position.
/// 2. for a type hierarchy item the supertype or subtype type hierarchy items are resolved.
pub enum TypeHierarchyPrepare {}

impl Request for TypeHierarchyPrepare {
    type Params = TypeHierarchyPrepareParams;
    type Result = Option<Vec<TypeHierarchyItem>>;
    const METHOD: &'static str = "textDocument/prepareTypeHierarchy";
}

/// The `typeHierarchy/supertypes` request is sent from the client to the server to resolve the
/// supertypes for a given type hierarchy item. Will return null if the server couldn’t infer a
/// valid type from item in the params. The request doesn’t define its own client and server
/// capabilities. It is only issued if a server registers for the
/// `textDocument/prepareTypeHierarchy` request.
pub enum TypeHierarchySupertypes {}

impl Request for TypeHierarchySupertypes {
    type Params = TypeHierarchySupertypesParams;
    type Result = Option<Vec<TypeHierarchyItem>>;
    const METHOD: &'static str = "typeHierarchy/supertypes";
}

/// The `typeHierarchy/subtypes` request is sent from the client to the server to resolve the
/// subtypes for a given type hierarchy item. Will return null if the server couldn’t infer a valid
/// type from item in the params. The request doesn’t define its own client and server capabilities.
/// It is only issued if a server registers for the textDocument/prepareTypeHierarchy request.
pub enum TypeHierarchySubtypes {}

impl Request for TypeHierarchySubtypes {
    type Params = TypeHierarchySubtypesParams;
    type Result = Option<Vec<TypeHierarchyItem>>;
    const METHOD: &'static str = "typeHierarchy/subtypes";
}

#[cfg(test)]
mod test {
    use super::*;

    fn fake_call<R>()
    where
        R: Request,
        R::Params: serde::Serialize,
        R::Result: serde::de::DeserializeOwned,
    {
    }

    macro_rules! check_macro {
        ($name:tt) => {
            // check whether the macro name matches the method
            assert_eq!(<lsp_request!($name) as Request>::METHOD, $name);
            // test whether type checking passes for each component
            fake_call::<lsp_request!($name)>();
        };
    }

    #[test]
    fn check_macro_definitions() {
        check_macro!("initialize");
        check_macro!("shutdown");

        check_macro!("window/showDocument");
        check_macro!("window/showMessageRequest");
        check_macro!("window/workDoneProgress/create");

        check_macro!("client/registerCapability");
        check_macro!("client/unregisterCapability");

        check_macro!("textDocument/willSaveWaitUntil");
        check_macro!("textDocument/completion");
        check_macro!("textDocument/hover");
        check_macro!("textDocument/signatureHelp");
        check_macro!("textDocument/declaration");
        check_macro!("textDocument/definition");
        check_macro!("textDocument/references");
        check_macro!("textDocument/documentHighlight");
        check_macro!("textDocument/documentSymbol");
        check_macro!("textDocument/codeAction");
        check_macro!("textDocument/codeLens");
        check_macro!("textDocument/documentLink");
        check_macro!("textDocument/rangeFormatting");
        check_macro!("textDocument/onTypeFormatting");
        check_macro!("textDocument/formatting");
        check_macro!("textDocument/rename");
        check_macro!("textDocument/documentColor");
        check_macro!("textDocument/colorPresentation");
        check_macro!("textDocument/foldingRange");
        check_macro!("textDocument/prepareRename");
        check_macro!("textDocument/implementation");
        check_macro!("textDocument/selectionRange");
        check_macro!("textDocument/typeDefinition");
        check_macro!("textDocument/moniker");
        check_macro!("textDocument/linkedEditingRange");
        check_macro!("textDocument/prepareCallHierarchy");
        check_macro!("textDocument/prepareTypeHierarchy");
        check_macro!("textDocument/semanticTokens/full");
        check_macro!("textDocument/semanticTokens/full/delta");
        check_macro!("textDocument/semanticTokens/range");
        check_macro!("textDocument/inlayHint");
        check_macro!("textDocument/inlineValue");
        check_macro!("textDocument/diagnostic");

        check_macro!("workspace/applyEdit");
        check_macro!("workspace/symbol");
        check_macro!("workspace/executeCommand");
        check_macro!("workspace/configuration");
        check_macro!("workspace/diagnostic");
        check_macro!("workspace/diagnostic/refresh");
        check_macro!("workspace/willCreateFiles");
        check_macro!("workspace/willRenameFiles");
        check_macro!("workspace/willDeleteFiles");
        check_macro!("workspace/workspaceFolders");
        check_macro!("workspace/semanticTokens/refresh");
        check_macro!("workspace/codeLens/refresh");
        check_macro!("workspace/inlayHint/refresh");
        check_macro!("workspace/inlineValue/refresh");

        check_macro!("callHierarchy/incomingCalls");
        check_macro!("callHierarchy/outgoingCalls");
        check_macro!("codeAction/resolve");
        check_macro!("codeLens/resolve");
        check_macro!("completionItem/resolve");
        check_macro!("documentLink/resolve");
        check_macro!("inlayHint/resolve");
        check_macro!("typeHierarchy/subtypes");
        check_macro!("typeHierarchy/supertypes");
        check_macro!("workspaceSymbol/resolve");
    }

    #[test]
    #[cfg(feature = "proposed")]
    fn check_proposed_macro_definitions() {}
}
