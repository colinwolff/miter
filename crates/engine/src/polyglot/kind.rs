//! What a declaration is, across the languages.

/// What a declaration is, in the four languages' shared set of declaration kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolKind {
    /// `mod`, `namespace`, `module`.
    Module,
    /// `struct`, `union`.
    Struct,
    /// `enum`.
    Enum,
    /// `trait`.
    Trait,
    /// `class`.
    Class,
    /// `interface`.
    Interface,
    /// `type` — a Rust `type_item` or a TypeScript `type_alias_declaration`.
    TypeAlias,
    /// A free function.
    Function,
    /// A method, or a function bound to a member (an exported hook, a class method).
    Method,
    /// A data member: an interface's `property_signature`, a class field.
    Property,
    /// Database table, view, or index.
    Table,
    /// OpenTofu/Terraform resource, module, or variable.
    InfraResource,
    /// Docker build stage, exposed port, or entrypoint.
    DockerStage,
    /// Kubernetes kind, service, or Litestream replication manifest.
    K8sManifest,
    /// A Markdown ATX heading: the document's own outline.
    Heading,
}

impl SymbolKind {
    /// Every kind, in report order.
    pub const ALL: [SymbolKind; 15] = [
        SymbolKind::Module,
        SymbolKind::Struct,
        SymbolKind::Enum,
        SymbolKind::Trait,
        SymbolKind::Class,
        SymbolKind::Interface,
        SymbolKind::TypeAlias,
        SymbolKind::Function,
        SymbolKind::Method,
        SymbolKind::Property,
        SymbolKind::Table,
        SymbolKind::InfraResource,
        SymbolKind::DockerStage,
        SymbolKind::K8sManifest,
        SymbolKind::Heading,
    ];

    /// Human-readable name, the spelling a report or a label uses.
    pub const fn name(self) -> &'static str {
        match self {
            SymbolKind::Module => "module",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::TypeAlias => "type_alias",
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Property => "property",
            SymbolKind::Table => "table",
            SymbolKind::InfraResource => "infra_resource",
            SymbolKind::DockerStage => "docker_stage",
            SymbolKind::K8sManifest => "k8s_manifest",
            SymbolKind::Heading => "heading",
        }
    }

    /// `true` for the kinds that introduce a *scope* — what `level` 2 names.
    pub const fn is_scope(self) -> bool {
        matches!(
            self,
            SymbolKind::Struct
                | SymbolKind::Enum
                | SymbolKind::Trait
                | SymbolKind::Class
                | SymbolKind::Interface
                | SymbolKind::TypeAlias
                | SymbolKind::Table
                | SymbolKind::InfraResource
                | SymbolKind::DockerStage
                | SymbolKind::K8sManifest
                | SymbolKind::Heading
        )
    }

    /// `true` for the kinds whose body is code rather than declarations.
    pub const fn is_callable(self) -> bool {
        matches!(self, SymbolKind::Function | SymbolKind::Method)
    }
}
