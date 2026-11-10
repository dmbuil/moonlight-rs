//! Internationalisation: English (US) and Spanish (Spain).
//!
//! Every UI string is declared once in the `translations!` table below with
//! both its English and Spanish text. The macro syntax makes it impossible to
//! add a key without both translations, so a missing translation is a
//! compile error rather than a runtime surprise.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Name of the cookie that remembers the chosen language.
pub const LANG_COOKIE: &str = "ml_lang";

/// UI language. Picked from the `ml_lang` cookie, else `Accept-Language`, the same
/// way on server and client so hydration matches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    EnUs,
    EsEs,
}

impl Lang {
    /// Every language, in the language menu's order.
    pub const ALL: [Lang; 2] = [Lang::EnUs, Lang::EsEs];

    /// BCP-47 tag, used for `<html lang>` and number/date formatting.
    pub fn tag(self) -> &'static str {
        match self {
            Lang::EnUs => "en-US",
            Lang::EsEs => "es-ES",
        }
    }

    /// Value stored in the language cookie.
    pub fn cookie_value(self) -> &'static str {
        match self {
            Lang::EnUs => "en",
            Lang::EsEs => "es",
        }
    }

    /// Name of the language, written in that language.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::EnUs => "English (US)",
            Lang::EsEs => "Español (España)",
        }
    }

    /// Parses a cookie value or a language tag (`es`, `es-ES`, `en_US`…).
    pub fn parse(s: &str) -> Option<Lang> {
        let s = s.trim().to_ascii_lowercase();
        if s.starts_with("es") {
            Some(Lang::EsEs)
        } else if s.starts_with("en") {
            Some(Lang::EnUs)
        } else {
            None
        }
    }

    /// Picks the best supported language from an `Accept-Language` header,
    /// honouring the order (q-values are ignored: browsers already sort by them).
    pub fn from_accept_language(header: &str) -> Lang {
        header
            .split(',')
            .filter_map(|part| part.split(';').next())
            .find_map(Lang::parse)
            .unwrap_or_default()
    }
}

macro_rules! translations {
    ($($key:ident => $en:literal, $es:literal;)*) => {
        /// Translation keys.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum K { $($key),* }

        impl K {
            /// Every key, for the test that checks nothing is left untranslated.
            pub const ALL: &'static [K] = &[$(K::$key),*];
        }

        fn en(k: K) -> &'static str { match k { $(K::$key => $en),* } }
        fn es(k: K) -> &'static str { match k { $(K::$key => $es),* } }
    };
}

translations! {
    // Chrome / navigation
    AppName => "Moonlight", "Moonlight";
    Project => "Project", "Proyecto";
    SelectProject => "Select a project", "Seleccione un proyecto";
    Namespace => "Namespace", "Espacio de nombres";
    Contents => "Contents", "Contenido";
    Objects => "Objects", "Objetos";
    Prefix => "Prefix", "Prefijo";
    PropertiesTarget => "Properties Target", "Destino de las propiedades";
    Title => "Title", "Título";
    AllowedValues => "Allowed Values", "Valores permitidos";
    MetadefDefinition => "Definition (JSON)", "Definición (JSON)";
    Prefixes => "Prefixes", "Prefijos";
    PrefixesHelp => "Per resource type (key), what goes before each property's name there, e.g. hw:. A new prefix removes the association and adds it back.", "Por tipo de recurso (clave), lo que va delante del nombre de cada propiedad en él, p. ej. hw:. Un prefijo nuevo quita la asociación y la vuelve a añadir.";
    PrefixNotAssociated => "A prefix for a resource type that isn't selected", "Un prefijo para un tipo de recurso no seleccionado";
    FilterFirstNote => "Give a filter to list the rows: this list waits for one before asking the cloud.", "Indica un filtro para ver las filas: esta lista espera uno antes de consultar la nube.";
    DeletedSuffix => "(Deleted)", "(Eliminado)";
    ImageOwnerHelp => "The project that owns the image; empty: this one.", "El proyecto propietario de la imagen; vacío: este.";
    SelectChoice => "— Select —", "— Seleccione —";
    Admin => "Admin", "Administración";
    Identity => "Identity", "Identidad";
    Projects => "Projects", "Proyectos";
    Users => "Users", "Usuarios";
    Groups => "Groups", "Grupos";
    Roles => "Roles", "Roles";
    ApplicationCredentials => "Application Credentials", "Credenciales de aplicación";
    Email => "Email", "Correo electrónico";
    DomainName => "Domain Name", "Nombre del dominio";
    EnabledCol => "Enabled", "Habilitado";
    RoleName => "Role Name", "Nombre del rol";
    RoleId => "Role ID", "ID del rol";
    GroupId => "Group ID", "ID del grupo";
    Expiration => "Expiration", "Caducidad";
    Never => "Never", "Nunca";
    DomainId => "Domain ID", "ID del dominio";
    PasswordExpiresAt => "Password Expires At", "La contraseña caduca el";
    LockPassword => "Lock password", "Contraseña bloqueada";
    PrimaryProject => "Primary Project", "Proyecto principal";
    RolesFromGroups => "Roles from Groups", "Roles desde grupos";
    CreateRole => "Create Role", "Crear rol";
    EditRole => "Edit Role", "Editar rol";
    DeleteRole => "Delete Role", "Eliminar rol";
    DeleteRoles => "Delete Roles", "Eliminar roles";
    DeleteRoleWarning => "Every assignment of the role, on every project and domain, is removed with it.", "Con él se eliminan todas sus asignaciones, en todos los proyectos y dominios.";
    RoleAssignments => "Role assignments", "Asignaciones de roles";
    ProjectOrDomain => "Project / Domain", "Proyecto / Dominio";
    Role => "Role", "Rol";
    Inherited => "Inherited", "Heredado";
    System => "System", "Sistema";
    GroupMembers => "Group Members", "Miembros del grupo";
    ImpliedRoles => "Implied Roles", "Roles implícitos";
    Quotas => "Quotas", "Cuotas";
    // Project › Quotas (roadmap 82)
    QuotaCol => "Quota", "Cuota";
    QuotaUsed => "Used", "En uso";
    QuotaReserved => "Reserved", "Reservado";
    QuotaLimit => "Limit", "Límite";
    EditQuotas => "Edit Quotas", "Editar cuotas";
    UpdateDefaults => "Update Defaults", "Actualizar valores predeterminados";
    ImportNamespace => "Import Namespace", "Importar espacio de nombres";
    NamespaceDefinition => "Namespace Definition", "Definición del espacio de nombres";
    NamespaceDefinitionHelp => "Glance's JSON for one namespace: namespace, display_name, properties, objects…", "El JSON de Glance de un espacio de nombres: namespace, display_name, properties, objects…";
    EditNamespace => "Edit Namespace", "Editar espacio de nombres";
    DisplayName => "Display Name", "Nombre visible";
    UpdateAssociations => "Update Associations", "Actualizar asociaciones";
    DeleteNamespace => "Delete Namespace", "Eliminar espacio de nombres";
    DeleteNamespaces => "Delete Namespaces", "Eliminar espacios de nombres";
    KeyManagerUsageNote => "Used counts only the secrets, containers and orders you can see. The project may hold more: Barbican hides those whose creator turned off project access, unless they were shared with you.", "En uso solo cuenta los secretos, contenedores y pedidos que puedes ver. El proyecto puede tener más: Barbican oculta aquellos cuyo creador desactivó el acceso del proyecto, salvo que te los hayan compartido.";
    QuotaLimitHelp => "-1 = unlimited. It can't be below what is used.", "-1 = ilimitado. No puede ser menor que lo que está en uso.";
    DefaultQuotaHelp => "-1 = unlimited. Applies to every project without its own quota.", "-1 = ilimitado. Se aplica a todo proyecto sin cuota propia.";
    MetadataForbidden => "You have no permission to change this metadata.", "No tienes permiso para cambiar estos metadatos.";
    QuotaForbidden => "You have no permission to edit quotas.", "No tienes permiso para editar cuotas.";
    QServerGroupMembers => "Server Group Members", "Miembros por grupo de servidores";
    QMetadataItems => "Metadata Items", "Elementos de metadatos";
    QInjectedFiles => "Injected Files", "Archivos inyectados";
    QInjectedFileContent => "Injected File Content (Bytes)", "Contenido de archivo inyectado (bytes)";
    QInjectedFilePath => "Length of Injected File Path", "Longitud de la ruta de archivo inyectado";
    QGigabytes => "Total Size of Volumes and Snapshots", "Tamaño total de volúmenes e instantáneas";
    QBackups => "Volume Backups", "Copias de seguridad de volúmenes";
    QBackupGigabytes => "Total Size of Backups", "Tamaño total de las copias de seguridad";
    QVolumeGroups => "Volume Groups", "Grupos de volúmenes";
    QPerVolumeGigabytes => "Maximum Size of One Volume", "Tamaño máximo de un volumen";
    QSubnets => "Subnets", "Subredes";
    QSubnetPools => "Subnet Pools", "Pools de subredes";
    QRbacPolicies => "RBAC Policies", "Políticas RBAC";
    QOrders => "Orders", "Pedidos";
    QCertificateAuthorities => "Certificate Authorities", "Autoridades de certificación";
    ViewQuotas => "See them on Project › Quotas", "Verlas en Proyecto › Cuotas";
    Method => "Method", "Método";
    Path => "Path", "Ruta";
    CreateProject => "Create Project", "Crear proyecto";
    EditProject => "Edit Project", "Editar proyecto";
    DeleteProject => "Delete Project", "Eliminar proyecto";
    DeleteProjects => "Delete Projects", "Eliminar proyectos";
    ManageMembers => "Manage Members", "Gestionar miembros";
    ModifyGroups => "Modify Groups", "Modificar grupos";
    CreateUser => "Create User", "Crear usuario";
    EditUser => "Edit User", "Editar usuario";
    DeleteUser => "Delete User", "Eliminar usuario";
    DeleteUsers => "Delete Users", "Eliminar usuarios";
    EnableUser => "Enable User", "Habilitar usuario";
    DisableUser => "Disable User", "Deshabilitar usuario";
    ConfirmPassword => "Confirm Password", "Confirmar contraseña";
    PasswordsDiffer => "The passwords do not match.", "Las contraseñas no coinciden.";
    CreateGroup => "Create Group", "Crear grupo";
    EditGroup => "Edit Group", "Editar grupo";
    DeleteGroup => "Delete Group", "Eliminar grupo";
    DeleteGroups => "Delete Groups", "Eliminar grupos";
    Domains => "Domains", "Dominios";
    SetDomainContext => "Set Domain Context", "Establecer contexto de dominio";
    ClearDomainContext => "Clear Domain Context", "Borrar contexto de dominio";
    DomainContextIs => "Domain context:", "Contexto de dominio:";
    DomainContextHelp => "The Projects, Users and Groups lists show this domain instead of yours.", "Las listas de proyectos, usuarios y grupos muestran este dominio en lugar del suyo.";
    SetActiveProject => "Set as Active Project", "Establecer como proyecto activo";
    Compute => "Compute", "Compute";
    VolumesGroup => "Volumes", "Volúmenes";
    NetworkGroup => "Network", "Red";
    Orchestration => "Orchestration", "Orquestación";
    Overview => "Overview", "Visión general";
    Instances => "Instances", "Instancias";
    Images => "Images", "Imágenes";
    KeyPairs => "Key Pairs", "Pares de claves";
    ServerGroups => "Server Groups", "Grupos de servidores";
    Volumes => "Volumes", "Volúmenes";
    Snapshots => "Snapshots", "Instantáneas";
    // Volume groups (roadmap 5); *Delete Group(s)* reuse the Identity keys.
    GroupSnapshots => "Group Snapshots", "Instantáneas de grupo";
    VolumeGroup => "Group", "Grupo";
    GroupType => "Group Type", "Tipo de grupo";
    VolumeTypes => "Volume Types", "Tipos de volumen";
    HasSnapshots => "Has Snapshots", "Tiene instantáneas";
    DeleteGroupSnapshot => "Delete Group Snapshot", "Eliminar instantánea de grupo";
    DeleteGroupSnapshots => "Delete Group Snapshots", "Eliminar instantáneas de grupo";
    DeleteGroupVolumes => "Delete Volumes", "Eliminar volúmenes";
    DeleteGroupVolumesHelp => "Also deletes every volume in the group. Without it, a group that still has volumes can't be deleted.", "Elimina también todos los volúmenes del grupo. Sin ello, un grupo que aún tiene volúmenes no se puede eliminar.";
    VolumeGroupTypesHelp => "The volume types the group accepts; its volumes must have one of them.", "Los tipos de volumen que acepta el grupo; sus volúmenes deben tener uno de ellos.";
    ManageVolumes => "Manage Volumes", "Gestionar volúmenes";
    ManageVolumesHelp => "Volumes without a group can be added if they have one of the group's volume types and are available or in use.", "Se pueden añadir volúmenes sin grupo que tengan uno de los tipos de volumen del grupo y estén disponibles o en uso.";
    RemoveGroupVolumes => "Remove Volumes from Group", "Quitar volúmenes del grupo";
    RemoveGroupVolumesHelp => "The volumes stay; only their membership in the group ends.", "Los volúmenes se conservan; solo dejan de pertenecer al grupo.";
    CloneGroup => "Clone Group", "Clonar grupo";
    CloneGroupHelp => "Creates a new group with a copy of each of this group's volumes.", "Crea un grupo nuevo con una copia de cada volumen de este grupo.";
    CreateGroupFromSnapshotHelp => "Creates a new group with a volume from each volume snapshot in this group snapshot.", "Crea un grupo nuevo con un volumen a partir de cada instantánea de volumen de esta instantánea de grupo.";
    Networks => "Networks", "Redes";
    Routers => "Routers", "Routers";
    SecurityGroups => "Security Groups", "Grupos de seguridad";
    FloatingIps => "Floating IPs", "IP flotantes";
    ApiAccess => "API Access", "Acceso a la API";
    Region => "Region", "Región";
    Language => "Language", "Idioma";
    SignOut => "Sign Out", "Cerrar sesión";
    ReadOnly => "Read-only", "Solo lectura";
    ReadOnlyHint => "Moonlight PoC is read-only: no actions can be triggered from here.", "La PoC de Moonlight es de solo lectura: desde aquí no se puede lanzar ninguna acción.";
    ComingSoon => "Not available in this PoC", "No disponible en esta PoC";
    SwitchProject => "Switch project", "Cambiar de proyecto";
    SearchProjects => "Search projects…", "Buscar proyectos…";

    // Generic table
    FilterPlaceholder => "Filter…", "Filtrar…";
    // Paged lists (roadmap 88)
    FilterBy => "Filter by", "Filtrar por";
    FilterApply => "Filter", "Filtrar";
    FilterSearch => "Search", "Buscar";
    FilterExact => "Exact value", "Valor exacto";
    FilterVolumeId => "Volume ID", "ID del volumen";
    FilterPoolId => "Pool ID", "ID del pool";
    FilterRouterId => "Router ID", "ID del router";
    FilterPortId => "Parent port ID", "ID del puerto padre";
    FilterAny => "— Any —", "— Cualquiera —";
    FilterIp6 => "IPv6 Address", "Dirección IPv6";
    PagePrev => "Previous", "Anterior";
    PageFirst => "First page", "Primera página";
    PageNext => "Next page", "Página siguiente";
    SortAllRows => "Sorts every row, on the server", "Ordena todas las filas, en el servidor";
    SortThisPage => "Sorts only the rows on this page", "Ordena solo las filas de esta página";
    CountAll => "Count all", "Contar todos";
    Counting => "Counting…", "Contando…";
    CountFailed => "Could not count", "No se pudo contar";
    NSelected => "selected", "seleccionados";
    NoItems => "No items to display.", "No hay elementos para mostrar.";
    Loading => "Loading…", "Cargando…";
    ErrorLoading => "Could not load data", "No se han podido cargar los datos";
    Refresh => "Refresh", "Actualizar";
    Yes => "Yes", "Sí";
    No => "No", "No";
    Unknown => "Unknown", "Desconocido";
    FetchedIn => "fetched in", "obtenido en";

    // Login
    LoginTitle => "Log in", "Iniciar sesión";
    LoginSubtitle => "Sign in with your OpenStack (Keystone) credentials.", "Accede con tus credenciales de OpenStack (Keystone).";
    Username => "User Name", "Nombre de usuario";
    Password => "Password", "Contraseña";
    Domain => "Domain", "Dominio";
    SignIn => "Sign In", "Acceder";
    SigningIn => "Signing in…", "Accediendo…";
    AuthenticateUsing => "Authenticate using", "Autenticar usando";
    KeystoneCredentials => "Keystone Credentials", "Credenciales de Keystone";
    FederatedApiAccessNote => "You signed in through single sign-on: API or CLI access needs the keystoneauth-websso plugin, and cannot use RC file.", "Has iniciado sesión con inicio de sesión único: el acceso por API o CLI necesita el plugin keystoneauth-websso y no puede usar el fichero RC.";
    FederatedApiAccessScripts => "For scripts without a browser, create an", "Para scripts sin navegador, crea una";
    ApplicationCredentialLink => "application credential", "credencial de aplicación";
    SsoNotStarted => "The single sign-on was not started from this browser, or took too long. Try again.", "El inicio de sesión único no se inició desde este navegador o tardó demasiado. Inténtalo de nuevo.";
    LoginFailed => "Invalid credentials, or Keystone could not be reached.", "Credenciales no válidas o no se ha podido contactar con Keystone.";
    NoProjects => "Your user has no projects assigned.", "Tu usuario no tiene proyectos asignados.";

    // Common columns
    Name => "Name", "Nombre";
    Id => "ID", "ID";
    Description => "Description", "Descripción";
    DnsName => "DNS Name", "Nombre DNS";
    Help => "Help", "Ayuda";
    ChangePassword => "Change Password", "Cambiar contraseña";
    DnsDomain => "DNS Domain", "Dominio DNS";
    Status => "Status", "Estado";
    Size => "Size", "Tamaño";
    Type => "Type", "Tipo";
    Created => "Created", "Creado";
    Age => "Age", "Antigüedad";
    AvailabilityZone => "Availability Zone", "Zona de disponibilidad";

    // Instances
    InstanceName => "Instance Name", "Nombre de la instancia";
    ImageName => "Image Name", "Nombre de la imagen";
    IpAddress => "IP Address", "Dirección IP";
    Flavor => "Flavor", "Sabor";
    KeyPair => "Key Pair", "Par de claves";
    Task => "Task", "Tarea";
    PowerState => "Power State", "Estado de energía";
    Floating => "floating", "flotante";
    Information => "Information", "Información";
    Specs => "Specs", "Especificaciones";
    IpAddresses => "IP Addresses", "Direcciones IP";
    Metadata => "Metadata", "Metadatos";
    VolumesAttached => "Volumes Attached", "Volúmenes adjuntos";
    Interfaces => "Interfaces", "Interfaces";
    ActionLog => "Action Log", "Registro de acciones";
    LogTab => "Log", "Registro";
    ConsoleTab => "Console", "Consola";
    ConsoleDisabled => "The instance log and console are disabled. Enable them with [console] enabled in moonlight.conf.", "El registro y la consola de la instancia están desactivados. Actívalos con [console] enabled en moonlight.conf.";
    ConsoleDryRun => "Dry run: Moonlight sends this request only with [writes] mode = on. It does not change the instance.", "Simulación: Moonlight solo envía esta petición con [writes] mode = on. No cambia la instancia.";
    LogLength => "Log Length", "Longitud del registro";
    Go => "Go", "Ir";
    ViewFullLog => "View Full Log", "Ver registro completo";
    LogEmpty => "The log is empty.", "El registro está vacío.";
    ConsoleOnlyActive => "The console is available only while the instance is active.", "La consola solo está disponible mientras la instancia está activa.";
    ConsoleOpenAlone => "Open the console alone", "Abrir solo la consola";
    ConsoleKeyboardHint => "If the console does not respond to the keyboard, click inside it first.", "Si la consola no responde al teclado, haz clic dentro de ella primero.";
    Ram => "RAM", "RAM";
    Vcpus => "VCPUs", "VCPUs";
    Disk => "Disk", "Disco";
    FlavorId => "Flavor ID", "ID del sabor";
    Locked => "Locked", "Bloqueada";
    Host => "Host", "Host";
    Fault => "Fault", "Fallo";
    Code => "Code", "Código";
    Message => "Message", "Mensaje";
    RequestId => "Request ID", "ID de petición";
    Action => "Action", "Acción";
    StartTime => "Start Time", "Hora de inicio";
    UserId => "User ID", "ID de usuario";
    UserName => "User Name", "Nombre de usuario";
    ProjectName => "Project Name", "Nombre del proyecto";
    ProjectId => "Project ID", "ID del proyecto";
    AuthUrl => "Authentication URL", "URL de autenticación";
    UserCredentials => "User Credentials", "Credenciales de usuario";
    ViewCredentials => "View Credentials", "Ver credenciales";
    HideCredentials => "Hide Credentials", "Ocultar credenciales";
    DownloadOpenRc => "OpenStack RC File", "Archivo RC de OpenStack";
    DownloadCloudsYaml => "OpenStack clouds.yaml File", "Archivo clouds.yaml de OpenStack";
    NoSecretsNote => "No password is included: the RC file asks for it, and clouds.yaml leaves it out.", "No se incluye ninguna contraseña: el archivo RC la pide y clouds.yaml la omite.";
    PortId => "Port ID", "ID de puerto";
    Network => "Network", "Red";
    FixedIps => "Fixed IPs", "IP fijas";
    MacAddress => "MAC Address", "Dirección MAC";
    BootFromVolume => "(booted from volume)", "(arrancada desde volumen)";
    NotAttached => "Not attached", "Sin adjuntar";
    NoneValue => "None", "Ninguno";

    // Images, key pairs, server groups
    Visibility => "Visibility", "Visibilidad";
    Protected => "Protected", "Protegida";
    DiskFormat => "Disk Format", "Formato de disco";
    Owner => "Owner", "Propietario";
    Fingerprint => "Fingerprint", "Huella";
    Policies => "Policies", "Políticas";
    Members => "Members", "Miembros";
    ImageTypeImage => "Image", "Imagen";
    ImageTypeSnapshot => "Snapshot", "Instantánea";

    // Volumes
    AttachedTo => "Attached To", "Adjunto a";
    Bootable => "Bootable", "Arrancable";
    Encrypted => "Encrypted", "Cifrado";
    VolumeName => "Volume Name", "Nombre del volumen";
    On => "on", "en";

    // Network
    Subnets => "Subnets Associated", "Subredes asociadas";
    Shared => "Shared", "Compartida";
    ProjectNetwork => "Project network", "Red del proyecto";
    Legend => "Legend", "Leyenda";
    External => "External", "Externa";
    AdminState => "Admin State", "Estado de administración";
    AdminUp => "UP", "ACTIVO";
    AdminDown => "DOWN", "INACTIVO";
    ExternalNetwork => "External Network", "Red externa";
    Rules => "Rules", "Reglas";
    MappedFixedIp => "Mapped Fixed IP Address", "IP fija asignada";
    Pool => "Pool", "Pool";

    // Overview
    LimitSummary => "Limit Summary", "Resumen de límites";
    UsageSummary => "Usage Summary", "Resumen de uso";
    UsagePeriodHelp => "Select a period of time to query its usage:", "Seleccione un periodo para consultar su uso:";
    DateFrom => "From", "Desde";
    DateTo => "To", "Hasta";
    Submit => "Submit", "Enviar";
    DownloadCsv => "Download CSV Summary", "Descargar resumen CSV";
    ActiveInstances => "Active Instances", "Instancias activas";
    ActiveRam => "Active RAM", "RAM activa";
    VcpuHours => "This Period's VCPU-Hours", "Horas de VCPU del periodo";
    GbHours => "This Period's GB-Hours", "Horas de GB del periodo";
    RamHours => "This Period's RAM-Hours", "Horas de RAM del periodo";
    VcpuHoursCol => "VCPU Hours", "Horas de VCPU";
    DiskGbHours => "Disk GB Hours", "Horas de GB de disco";
    MemoryMbHours => "Memory MB Hours", "Horas de MB de memoria";
    DhcpAgents => "DHCP Agents", "Agentes DHCP";
    ProviderNetworkType => "Provider Network Type", "Tipo de red del proveedor";
    Ipv6Modes => "IPv6 Address Configuration Mode", "Modo de configuración de direcciones IPv6";
    Ipv6ModeNone => "No options specified (Default)", "Sin opciones (predeterminado)";
    Ipv6ModeSlaacRouter => "SLAAC: Address discovered from OpenStack Router", "SLAAC: dirección obtenida del router de OpenStack";
    Ipv6ModeSlaacExternal => "SLAAC: Address discovered from an external router", "SLAAC: dirección obtenida de un router externo";
    Ipv6ModeStateful => "DHCPv6 stateful: Address discovered from OpenStack DHCP", "DHCPv6 con estado: dirección obtenida del DHCP de OpenStack";
    Ipv6ModeStateless => "DHCPv6 stateless: Address discovered from OpenStack Router and additional information from OpenStack DHCP", "DHCPv6 sin estado: dirección obtenida del router de OpenStack e información adicional del DHCP de OpenStack";
    HostRoutesHelp => "Extra routes for the instances, one per line: destination CIDR, next hop (e.g. 192.168.200.0/24,10.56.1.254).", "Rutas adicionales para las instancias, una por línea: CIDR de destino, siguiente salto (p. ej. 192.168.200.0/24,10.56.1.254).";
    AzHints => "Availability Zone Hints", "Sugerencias de zona de disponibilidad";
    AzHintsHelp => "Zones where Neutron should schedule the network's DHCP and routers; none lets it choose.", "Zonas donde Neutron debería programar el DHCP y los routers de la red; ninguna le deja elegir.";
    CreateFlavor => "Create Flavor", "Crear sabor";
    DeleteFlavor => "Delete Flavor", "Eliminar sabor";
    DeleteFlavors => "Delete Flavors", "Eliminar sabores";
    FlavorInfo => "Flavor Information", "Información del sabor";
    FlavorInfoHelp => "Flavors define the sizes for RAM, disk, number of cores, and other resources an instance gets.", "Los sabores definen el tamaño de RAM, disco, número de núcleos y otros recursos que recibe una instancia.";
    FlavorIdHelp => "Empty or `auto`: Nova generates a UUID.", "Vacío o `auto`: Nova genera un UUID.";
    RamMb => "RAM (MB)", "RAM (MB)";
    RootDiskGb => "Root Disk (GB)", "Disco raíz (GB)";
    EphemeralDiskGb => "Ephemeral Disk (GB)", "Disco efímero (GB)";
    SwapDiskMb => "Swap Disk (MB)", "Disco de intercambio (MB)";
    EditFlavor => "Edit Flavor", "Editar sabor";
    EditFlavorRecreates => "Nova can't change a flavor: saving deletes it and creates it again with the same ID, extra specs and access. Running instances keep their size.", "Nova no puede cambiar un sabor: al guardar se borra y se vuelve a crear con el mismo ID, especificaciones extra y acceso. Las instancias en marcha conservan su tamaño.";
    FlavorAccessStep => "Flavor Access", "Acceso al sabor";
    FlavorAccessHelp => "Pick projects to make the flavor private to them; none makes it public.", "Elija proyectos para que el sabor sea privado para ellos; ninguno lo hace público.";
    ModifyAccess => "Modify Access", "Modificar acceso";
    CreateAggregate => "Create Host Aggregate", "Crear agregado de hosts";
    EditAggregate => "Edit Host Aggregate", "Editar agregado de hosts";
    DeleteAggregate => "Delete Host Aggregate", "Eliminar agregado de hosts";
    DeleteAggregates => "Delete Host Aggregates", "Eliminar agregados de hosts";
    ManageHosts => "Manage Hosts", "Gestionar hosts";
    AggregateZoneHelp => "The availability zone the aggregate's hosts form; empty for none.", "La zona de disponibilidad que forman los hosts del agregado; vacía para ninguna.";
    DisableService => "Disable Service", "Deshabilitar servicio";
    EnableService => "Enable Service", "Habilitar servicio";
    DisableReason => "Reason", "Motivo";
    MigrateInstance => "Migrate Instance", "Migrar instancia";
    LiveMigrateInstance => "Live Migrate Instance", "Migrar instancia en caliente";
    NewHost => "New Host", "Nuevo host";
    NewHostHelp => "Empty: the scheduler picks one.", "Vacío: el planificador elige uno.";
    BlockMigration => "Block Migration", "Migración de bloques";
    UpdateVolumeStatus => "Update Volume Status", "Actualizar estado del volumen";
    CreateVolumeType => "Create Volume Type", "Crear tipo de volumen";
    EditVolumeType => "Edit Volume Type", "Editar tipo de volumen";
    DeleteVolumeType => "Delete Volume Type", "Eliminar tipo de volumen";
    DeleteVolumeTypes => "Delete Volume Types", "Eliminar tipos de volumen";
    ViewExtraSpecs => "View Extra Specs", "Ver especificaciones adicionales";
    ExtraSpecs => "Extra Specs", "Especificaciones adicionales";
    ExtraSpecsHelp => "Scheduler hints for this type, e.g. volume_backend_name.", "Indicaciones para el planificador de este tipo, p. ej. volume_backend_name.";
    ManageQosAssociation => "Manage QoS Spec Association", "Gestionar asociación de especificación QoS";
    QosSpec => "QoS Spec", "Especificación QoS";
    CreateEncryption => "Create Encryption", "Crear cifrado";
    UpdateEncryption => "Update Encryption", "Actualizar cifrado";
    DeleteEncryption => "Delete Encryption", "Eliminar cifrado";
    EncryptionProviderHelp => "The encryptor: luks, plain, or a class such as nova.volume.encryptors.luks.LuksEncryptor.", "El cifrador: luks, plain o una clase como nova.volume.encryptors.luks.LuksEncryptor.";
    ControlLocation => "Control Location", "Ubicación del control";
    ControlLocationHelp => "front-end: Nova encrypts; back-end: the storage does.", "front-end: cifra Nova; back-end: cifra el almacenamiento.";
    Cipher => "Cipher", "Algoritmo de cifrado";
    KeySize => "Key Size (bits)", "Tamaño de clave (bits)";
    CreateQosSpec => "Create QoS Spec", "Crear especificación QoS";
    ManageSpecs => "Manage Specs", "Gestionar especificaciones";
    EditConsumer => "Edit Consumer", "Editar consumidor";
    DeleteQosSpec => "Delete QoS Spec", "Eliminar especificación QoS";
    QosConsumerHelp => "Where the limits apply: back-end (the storage), front-end (the hypervisor) or both.", "Dónde se aplican los límites: back-end (el almacenamiento), front-end (el hipervisor) o ambos.";
    CreateGroupType => "Create Group Type", "Crear tipo de grupo";
    EditGroupType => "Edit Group Type", "Editar tipo de grupo";
    DeleteGroupType => "Delete Group Type", "Eliminar tipo de grupo";
    DeleteGroupTypes => "Delete Group Types", "Eliminar tipos de grupo";
    ViewSpecs => "View Specs", "Ver especificaciones";
    UpdateSnapshotStatus => "Update Status", "Actualizar estado";
    ResetStatusHelp => "Changes only Cinder's record, not the back end: use it to recover a resource stuck in a state.", "Cambia solo el registro de Cinder, no el back end: sirve para recuperar un recurso atascado en un estado.";
    MigrateVolume => "Migrate Volume", "Migrar volumen";
    CurrentHost => "Current Host", "Host actual";
    DestinationHost => "Destination Host", "Host de destino";
    ForceHostCopy => "Force Host Copy", "Forzar copia por el host";
    ForceHostCopyHelp => "Copy the data through the Cinder host instead of the driver's own migration.", "Copiar los datos a través del host de Cinder en lugar de la migración propia del driver.";
    ManageVolume => "Manage Volume", "Gestionar volumen";
    ManageVolumeHelp => "Bring a volume that exists on a storage back end, but not in Cinder, into the current project.", "Incorporar al proyecto actual un volumen que existe en un back end de almacenamiento pero no en Cinder.";
    Identifier => "Identifier", "Identificador";
    IdentifierHelp => "The volume's name or ID on the back end.", "El nombre o ID del volumen en el back end.";
    IdentifierType => "Identifier Type", "Tipo de identificador";
    BackendName => "Name", "Nombre";
    BackendId => "ID", "ID";
    VolumeHostHelp => "The Cinder host the volume is on: host@backend-name#pool.", "El host de Cinder en el que está el volumen: host@nombre-backend#pool.";
    UnmanageVolume => "Unmanage Volume", "Dejar de gestionar volumen";
    UnmanageVolumes => "Unmanage Volumes", "Dejar de gestionar volúmenes";
    UnmanageHelp => "Cinder forgets the volume; the back end keeps its data.", "Cinder olvida el volumen; el back end conserva sus datos.";
    BlockMigrationAuto => "Automatic (copy disks only without shared storage)", "Automática (copiar discos solo sin almacenamiento compartido)";
    RouterAzHintsHelp => "Zones where Neutron should schedule the router; none lets it choose.", "Zonas donde Neutron debería programar el router; ninguna le deja elegir.";
    UseServerDefault => "Use Server Default", "Usar el valor del servidor";
    RouterType => "Router Type", "Tipo de router";
    Distributed => "Distributed", "Distribuido";
    Centralized => "Centralized", "Centralizado";
    HaMode => "High Availability Mode", "Modo de alta disponibilidad";
    HaEnable => "Enable HA mode", "Habilitar modo HA";
    HaDisable => "Disable HA mode", "Deshabilitar modo HA";
    HighAvailability => "High Availability", "Alta disponibilidad";
    RouterChangeWhileDown => "Neutron changes this only while the router's admin state is down.", "Neutron solo lo cambia con el estado de administración del router desactivado.";
    RouterSetDownFirst => "The router is up: save it with Enable Admin State unticked first, then change this.", "El router está activo: guárdalo antes con Habilitar estado de administración desmarcado y cambia esto después.";
    MtuHelp => "Maximum transmission unit in bytes; empty uses the physical network's.", "Unidad máxima de transmisión en bytes; vacío usa la de la red física.";
    ProviderNetworkTypeHelp => "The physical mechanism by which the virtual network is implemented.", "El mecanismo físico con el que se implementa la red virtual.";
    PhysicalNetwork => "Physical Network", "Red física";
    SegmentationIdHelp => "VLAN 1–4094, VXLAN and Geneve 1–16777215; empty lets Neutron choose.", "VLAN 1–4094, VXLAN y Geneve 1–16777215; vacío deja que Neutron lo elija.";
    AdminNetworkStepHelp => "Create a network for any project, on a provider network; you can add its first subnet in the next steps.", "Cree una red para cualquier proyecto, sobre una red del proveedor; puede añadir su primera subred en los pasos siguientes.";
    FlavorAccess => "Projects that can use this flavor", "Proyectos que pueden usar este sabor";
    VolumeSnapshots => "Volume Snapshots", "Instantáneas de volumen";
    VolumeStorage => "Volume Storage", "Almacenamiento de volúmenes";
    SecurityGroupRules => "Security Group Rules", "Reglas de grupos de seguridad";
    Ports => "Ports", "Puertos";


    // Actions & dialogs
    Actions => "Actions", "Acciones";
    More => "More actions", "Más acciones";
    Create => "Create", "Crear";
    Edit => "Edit", "Editar";
    Delete => "Delete", "Eliminar";
    Cancel => "Cancel", "Cancelar";
    Close => "Close", "Cerrar";
    Save => "Save", "Guardar";
    Working => "Working…", "Procesando…";
    RequiredField => "required", "obligatorio";
    FieldRequired => "This field is required.", "Este campo es obligatorio.";
    YouSelected => "You have selected:", "Has seleccionado:";
    IUnderstand => "Yes, I understand this cannot be undone.", "Sí, entiendo que no se puede deshacer.";
    CannotBeUndone => "Please confirm your selection. This action cannot be undone.", "Confirma tu selección. Esta acción no se puede deshacer.";
    ModifyQuotas => "Modify Quotas", "Modificar cuotas";
    KeyManagerOtherProjectNote => "Barbican counts nothing for another project, so Used is empty. Quotas without their own value follow the cloud's defaults, listed only when Moonlight can tell them.", "Barbican no cuenta nada de otro proyecto, así que Usado queda vacío. Las cuotas sin valor propio siguen los valores por defecto de la nube, listados solo cuando Moonlight puede saberlos.";
    DryRunTitle => "Dry run: nothing was sent", "Simulación: no se ha enviado nada";
    JobKeepTabOpen => "Keep this tab open: closing it stops the items not yet sent.", "Mantenga esta pestaña abierta: si la cierra, los elementos pendientes no se envían.";
    JobRequests => "Requests", "Peticiones";
    DryRunExplain => "Moonlight is in dry-run mode. These are the exact requests the OpenStack SDK built for this action:", "Moonlight está en modo simulación. Estas son las peticiones exactas que el SDK de OpenStack ha construido para esta acción:";
    ActionDone => "Done. OpenStack accepted the request.", "Hecho. OpenStack ha aceptado la petición.";
    WritesOff => "Writes are disabled on this Moonlight instance.", "Las escrituras están desactivadas en esta instancia de Moonlight.";
    DryRunBadge => "Dry-run", "Simulación";
    DryRunHint => "Actions build their OpenStack requests but never send them.", "Las acciones construyen sus peticiones a OpenStack pero nunca las envían.";
    AnyOption => "Any", "Cualquiera";
    NoneOption => "— None —", "— Ninguno —";
    LoadingChoices => "Loading options…", "Cargando opciones…";

    // Instances
    LaunchInstance => "Launch Instance", "Lanzar instancia";
    EditInstance => "Edit Instance", "Editar instancia";
    CreateSnapshot => "Create Snapshot", "Crear instantánea";
    SnapshotName => "Snapshot Name", "Nombre de la instantánea";
    AttachVolume => "Attach Volume", "Adjuntar volumen";
    DetachVolume => "Detach Volume", "Desadjuntar volumen";
    PauseInstance => "Pause Instance", "Pausar instancia";
    UnpauseInstance => "Resume Instance", "Reanudar instancia";
    SuspendInstance => "Suspend Instance", "Suspender instancia";
    ResumeInstance => "Resume Suspended Instance", "Reanudar instancia suspendida";
    LockInstance => "Lock Instance", "Bloquear instancia";
    UnlockInstance => "Unlock Instance", "Desbloquear instancia";
    SoftReboot => "Soft Reboot Instance", "Reinicio suave de la instancia";
    HardReboot => "Hard Reboot Instance", "Reinicio forzado de la instancia";
    ShutOff => "Shut Off Instance", "Apagar instancia";
    StartInstance => "Start Instance", "Iniciar instancia";
    DeleteInstance => "Delete Instance", "Eliminar instancia";
    Count => "Count", "Número de instancias";
    BootSource => "Select Boot Source", "Origen de arranque";
    SourceItem => "Source", "Origen";
    BootImage => "Image", "Imagen";
    BootVolume => "Volume", "Volumen";
    BootInstanceSnapshot => "Instance Snapshot", "Instantánea de instancia";
    // Wizard
    Steps => "Steps", "Pasos";
    Back => "Back", "Atrás";
    Next => "Next", "Siguiente";
    Allocated => "Allocated", "Asignados";
    Available => "Available", "Disponibles";
    SelectOneHint => "Select an item from the available items below.", "Seleccione un elemento de los disponibles más abajo.";
    SelectManyHint => "Select one or more items from the available items below.", "Seleccione uno o varios elementos de los disponibles más abajo.";
    NothingToChoose => "There is nothing to choose from.", "No hay nada que elegir.";
    AllAllocated => "Every item is already selected.", "Ya están seleccionados todos los elementos.";
    Allocate => "Allocate", "Asignar";
    Deallocate => "Remove", "Quitar";
    MoveUp => "Move up", "Subir";
    NoMatches => "No matching items", "Ningún elemento coincide";
    Format => "Format", "Formato";
    TotalDisk => "Total Disk", "Disco total";
    EphemeralDisk => "Ephemeral Disk", "Disco efímero";
    Public => "Public", "Público";
    SubnetsAssociated => "Subnets Associated", "Subredes asociadas";
    Up => "Up", "Activo";
    Down => "Down", "Inactivo";
    StepDetails => "Details", "Detalles";
    StepSource => "Source", "Origen";
    StepNetworkPorts => "Network Ports", "Puertos de red";
    StepConfiguration => "Configuration", "Configuración";
    StepServerGroups => "Server Groups", "Grupos de servidores";
    StepSubnetDetails => "Subnet Details", "Detalles de la subred";
    DetailsHelp => "Give the instance a name and, optionally, an availability zone. Raise the count to create several identical instances.", "Indique el nombre de la instancia y, si quiere, una zona de disponibilidad. Aumente el número para crear varias instancias iguales.";
    SourceHelp => "The template the instance starts from: an image, an instance snapshot, a volume or a volume snapshot. With an image you can create a new persistent volume for it.", "La plantilla de la que parte la instancia: una imagen, una instantánea de instancia, un volumen o una instantánea de volumen. Con una imagen puede crear un volumen persistente nuevo.";
    FlavorHelp => "The flavor sets the vCPUs, memory and disk of the instance.", "El sabor fija las vCPU, la memoria y el disco de la instancia.";
    NetworksHelp => "Networks the instance connects to, in this order. You can use ports instead, or both.", "Redes a las que se conecta la instancia, en este orden. Puede usar puertos en su lugar, o ambos.";
    PortsHelp => "Existing ports to attach, after the networks.", "Puertos existentes que se conectan, después de las redes.";
    SecurityGroupsHelp => "Security groups applied to the instance.", "Grupos de seguridad aplicados a la instancia.";
    KeyPairHelp => "The key pair lets you log in over SSH. Create or import one in Key Pairs.", "El par de claves permite entrar por SSH. Cree o importe uno en Pares de claves.";
    ConfigurationHelp => "Customise the instance once it boots: the customization script is passed as user data (cloud-init).", "Personalice la instancia al arrancar: el script de personalización se pasa como user data (cloud-init).";
    ServerGroupsHelp => "Place the instance in a server group to apply its (anti-)affinity policy.", "Coloque la instancia en un grupo de servidores para aplicar su política de (anti)afinidad.";
    NetworkStepHelp => "Create a network; you can add its first subnet in the next steps.", "Cree una red; puede añadir su primera subred en los pasos siguientes.";
    SubnetStepHelp => "The subnet's address range and gateway.", "El rango de direcciones y la puerta de enlace de la subred.";
    SubnetDetailsHelp => "DHCP, allocation pools and DNS servers of the subnet.", "DHCP, rangos de asignación y servidores DNS de la subred.";
    CustomizationScript => "Customization Script", "Script de personalización";
    ConfigDrive => "Configuration Drive", "Unidad de configuración";
    DiskPartition => "Disk Partition", "Particionado de disco";
    Automatic => "Automatic", "Automático";
    Manual => "Manual", "Manual";
    NetworkOrPort => "Select at least one network or port", "Seleccione al menos una red o un puerto";
    LoadFromFile => "Load from a file", "Cargar desde un archivo";
    ScriptSize => "Script size", "Tamaño del script";
    BytesOf => "bytes of", "bytes de";
    FileTooBig => "The file is too large", "El archivo es demasiado grande";
    FileNotText => "The file is not text (UTF-8)", "El archivo no es texto (UTF-8)";
    UserDataTooBig => "The customization script is larger than 16 KB", "El script de personalización supera los 16 KB";
    BootVolumeSnapshot => "Volume Snapshot", "Instantánea de volumen";
    CreateNewVolume => "Create New Volume", "Crear volumen nuevo";
    VolumeSizeGb => "Volume Size (GB)", "Tamaño del volumen (GB)";
    DeleteOnTerminate => "Delete Volume on Instance Delete", "Eliminar el volumen al eliminar la instancia";
    ServerGroup => "Server Group", "Grupo de servidores";
    Volume => "Volume", "Volumen";
    Instance => "Instance", "Instancia";

    // Images, key pairs, server groups
    EditImage => "Edit Image", "Editar imagen";
    CreateImage => "Create Image", "Crear imagen";
    ImageSource => "Image Source", "Origen de la imagen";
    ImageSourceUrl => "URL", "URL";
    ImageImportFailed => "Import failed", "Importación fallida";
    ImageDetails => "Image Details", "Detalles de la imagen";
    ImageSourceFile => "File", "Archivo";
    ImageFileHelp => "The browser sends the file to the image service; keep this page open until it finishes.", "El navegador envía el archivo al servicio de imágenes; mantén esta página abierta hasta que termine.";
    ChooseFile => "Choose a file…", "Elegir un archivo…";
    ImageFileTooBig => "The file is larger than the allowed", "El archivo supera el máximo permitido de";
    Uploading => "Uploading the image", "Subiendo la imagen";
    UploadFailedTitle => "The upload did not finish", "La subida no terminó";
    UploadFailedText => "The image was created but has no data: it stays queued. You can delete it or upload its data some other way.", "La imagen se creó pero no tiene datos: queda en cola. Puedes eliminarla o subir sus datos de otra forma.";
    CancelUpload => "Cancel upload", "Cancelar subida";
    ImageSourceNone => "None (create the record only)", "Ninguno (solo crear el registro)";
    ImageSourceNoneHelp => "With None, the image stays queued until its data is uploaded some other way.", "Con Ninguno, la imagen queda en cola hasta que se suban sus datos de otra forma.";
    ImageUrlHelp => "The image service downloads it: an http:// or https:// address it can reach, to a disk image in the format below.", "El servicio de imágenes la descarga: una dirección http:// o https:// que pueda alcanzar, a una imagen de disco en el formato indicado.";
    UpdateMetadata => "Update Metadata", "Actualizar metadatos";
    ImageId => "Image ID", "ID de imagen";
    VirtualSize => "Virtual Size", "Tamaño virtual";
    Tags => "Tags", "Etiquetas";
    VisibilityPrivate => "Private", "Privada";
    VisibilityShared => "Shared", "Compartida";
    VisibilityCommunity => "Community", "Comunidad";
    VisibilityPublic => "Public", "Pública";
    PolicyAffinity => "Affinity", "Afinidad";
    PolicyAntiAffinity => "Anti Affinity", "Antiafinidad";
    PolicySoftAffinity => "Soft Affinity", "Afinidad flexible";
    PolicySoftAntiAffinity => "Soft Anti Affinity", "Antiafinidad flexible";
    KernelId => "Kernel ID", "ID del kernel";
    RamdiskId => "Ramdisk ID", "ID del ramdisk";
    Architecture => "Architecture", "Arquitectura";
    ArchitectureHelp => "e.g. x86_64 or aarch64; empty removes it.", "p. ej. x86_64 o aarch64; vacío lo quita.";
    MinDiskGb => "Minimum Disk (GB)", "Disco mínimo (GB)";
    MinRamMb => "Minimum RAM (MB)", "RAM mínima (MB)";
    MetadataHelp => "Keys and values up to 255 characters. Removing a row deletes that key.", "Claves y valores de hasta 255 caracteres. Quitar una fila borra esa clave.";
    Key => "Key", "Clave";
    Value => "Value", "Valor";
    Remove => "Remove", "Quitar";
    AddEntry => "Add", "Añadir";
    AddCustom => "Add custom", "Añadir personalizado";
    AddFromCatalog => "Add from catalog…", "Añadir del catálogo…";
    NoMetadata => "No metadata.", "Sin metadatos.";
    ShowDetails => "Show details", "Mostrar detalles";
    HideDetails => "Hide details", "Ocultar detalles";
    NothingChanged => "Nothing changed.", "No ha cambiado nada.";
    DeleteImage => "Delete Image", "Eliminar imagen";
    ImportPublicKey => "Import Public Key", "Importar clave pública";
    CreateKeyPair => "Create Key Pair", "Crear par de claves";
    GeneratedKeyHelp => "An Ed25519 SSH key is generated on the Moonlight server; only the public key is sent to OpenStack. You download the private key once. For an X509 certificate, use Import Public Key.", "Se genera una clave SSH Ed25519 en el servidor de Moonlight; solo la clave pública se envía a OpenStack. La clave privada se descarga una única vez. Para un certificado X509, usa Importar clave pública.";
    KeyType => "Key Type", "Tipo de clave";
    SshKey => "SSH Key", "Clave SSH";
    X509Certificate => "X509 Certificate", "Certificado X509";
    PublicKeyHelp => "SSH key: one line starting with ssh-, ecdsa- or sk-. X509 certificate: the PEM text, from -----BEGIN CERTIFICATE----- to -----END CERTIFICATE-----.", "Clave SSH: una línea que empieza por ssh-, ecdsa- o sk-. Certificado X509: el texto PEM, de -----BEGIN CERTIFICATE----- a -----END CERTIFICATE-----.";
    SecretTitle => "Download your private key now", "Descargue ahora su clave privada";
    SecretExplain => "It is not stored anywhere and will not be shown again.", "No se guarda en ningún sitio y no se volverá a mostrar.";
    DownloadFile => "Download", "Descargar";
    DeleteKeyPair => "Delete Key Pair", "Eliminar par de claves";
    PublicKey => "Public Key", "Clave pública";
    CreateServerGroup => "Create Server Group", "Crear grupo de servidores";
    DeleteServerGroup => "Delete Server Group", "Eliminar grupo de servidores";
    Policy => "Policy", "Política";
    Checksum => "Checksum", "Suma de verificación";
    ContainerFormat => "Container Format", "Formato de contenedor";
    MinDisk => "Min. Disk", "Disco mínimo";
    MinRam => "Min. RAM", "RAM mínima";
    Updated => "Updated", "Actualizado";
    CustomProperties => "Custom Properties", "Propiedades personalizadas";

    // Volumes & snapshots
    CreateVolume => "Create Volume", "Crear volumen";
    EditVolume => "Edit Volume", "Editar volumen";
    ExtendVolume => "Extend Volume", "Ampliar volumen";
    NewSizeGb => "New Size (GiB)", "Tamaño nuevo (GiB)";
    DeleteVolume => "Delete Volume", "Eliminar volumen";
    VolumeSource => "Volume Source", "Origen del volumen";
    SourceEmpty => "No source, empty volume", "Sin origen, volumen vacío";
    VolumeType => "Type", "Tipo";
    SizeGb => "Size (GiB)", "Tamaño (GiB)";
    ForceSnapshot => "Force (snapshot an attached volume)", "Forzar (instantánea de un volumen adjunto)";
    EditSnapshot => "Edit Snapshot", "Editar instantánea";
    DeleteSnapshot => "Delete Volume Snapshot", "Eliminar instantánea de volumen";
    Multiattach => "Multiattach", "Multiadjunto";
    ImageMetadata => "Volume Image Metadata", "Metadatos de imagen del volumen";
    SourceSnapshot => "Source Snapshot", "Instantánea de origen";

    // Networks, subnets, ports
    CreateNetwork => "Create Network", "Crear red";
    EditNetwork => "Edit Network", "Editar red";
    DeleteNetwork => "Delete Network", "Eliminar red";
    NetworkName => "Network Name", "Nombre de la red";
    EnableAdminState => "Enable Admin State", "Habilitar estado de administración";
    EnableSnat => "Enable SNAT", "Habilitar SNAT";
    WithSubnet => "Create Subnet", "Crear subred";
    CreateSubnet => "Create Subnet", "Crear subred";
    EditSubnet => "Edit Subnet", "Editar subred";
    DeleteSubnet => "Delete Subnet", "Eliminar subred";
    SubnetName => "Subnet Name", "Nombre de la subred";
    NetworkAddress => "Network Address", "Dirección de red";
    IpVersion => "IP Version", "Versión IP";
    GatewayIp => "Gateway IP", "IP de la puerta de enlace";
    GatewayAuto => "Leave empty to use the first address of the network.", "Déjalo vacío para usar la primera dirección de la red.";
    DisableGateway => "Disable Gateway", "Desactivar puerta de enlace";
    EnableDhcp => "Enable DHCP", "Habilitar DHCP";
    DnsNameServers => "DNS Name Servers", "Servidores DNS";
    OnePerLine => "One per line.", "Uno por línea.";
    AllocationPools => "Allocation Pools", "Pools de asignación";
    PoolsHelp => "One \"start,end\" pair per line.", "Un par \"inicio,fin\" por línea.";
    HostRoutes => "Host Routes", "Rutas de host";
    Subnet => "Subnet", "Subred";
    CreatePort => "Create Port", "Crear puerto";
    EditPort => "Edit Port", "Editar puerto";
    DeletePort => "Delete Port", "Eliminar puerto";
    FixedIpOptional => "Fixed IP Address (optional)", "Dirección IP fija (opcional)";
    Mtu => "MTU", "MTU";
    PortSecurity => "Port Security", "Seguridad de puertos";
    DeviceOwner => "Device Owner", "Propietario del dispositivo";
    AttachedDevice => "Attached Device", "Dispositivo adjunto";
    VnicType => "VNIC Type", "Tipo de VNIC";
    StepInfo => "Info", "Información";
    EditPortInfoHelp => "You may update the editable properties of your port here. Turning Port Security off also removes the port's security groups: no filtering is applied to its traffic.", "Aquí puede actualizar las propiedades editables del puerto. Desactivar la seguridad de puertos también quita sus grupos de seguridad: su tráfico no se filtra.";
    EditPortGroupsHelp => "The security groups applied to this port. Only available while Port Security is on.", "Los grupos de seguridad aplicados a este puerto. Solo disponible con la seguridad de puertos activada.";
    VnicNormal => "Normal", "Normal";
    VnicDirect => "Direct", "Directo";
    VnicDirectPhysical => "Direct Physical", "Físico directo";
    VnicMacvtap => "MacVTap", "MacVTap";
    VnicBaremetal => "Bare Metal", "Bare metal";
    VnicVirtioForwarder => "Virtio Forwarder", "Virtio Forwarder";
    VnicVdpa => "vDPA", "vDPA";
    VnicSmartNic => "Smart NIC", "Smart NIC";
    VnicRemoteManaged => "Remote Managed", "Gestionado remotamente";

    // Orchestration (Heat, roadmap 62)
    Stacks => "Stacks", "Pilas";
    StackTopology => "Topology", "Topología";
    StackResources => "Resources", "Recursos";
    StackEvents => "Events", "Eventos";
    StackTemplate => "Template", "Plantilla";
    StackOutputs => "Outputs", "Salidas";
    StackParameters => "Parameters", "Parámetros";
    StatusReason => "Status Reason", "Motivo del estado";
    StackTimeout => "Timeout", "Tiempo de espera";
    StackRollback => "Rollback on Failure", "Revertir si falla";
    ParentStack => "Parent Stack", "Pila padre";
    StackResource => "Resource", "Recurso";
    DependsOn => "Depends On", "Depende de";
    StackNode => "Stack", "Pila";
    LegendDependsOn => "Depends on", "Depende de";
    LegendInStack => "Part of the stack (nothing depends on it)", "Parte de la pila (nada depende de él)";
    LegendComplete => "Complete", "Completado";
    LegendInProgress => "In progress", "En curso";
    LegendFailed => "Failed", "Fallido";
    LegendInactive => "Not created, suspended or deleted", "Sin crear, suspendido o eliminado";
    RequiredBy => "Required By", "Requerido por";
    EventTime => "Time", "Hora";
    TopologyAsList => "Each resource with the resources it depends on and those that depend on it.", "Cada recurso con los recursos de los que depende y los que dependen de él.";
    NoStackResources => "This stack has no resources.", "Esta pila no tiene recursos.";
    NoStackOutputs => "This stack has no outputs.", "Esta pila no tiene salidas.";
    CheckStack => "Check Stack", "Comprobar pila";
    SuspendStack => "Suspend Stack", "Suspender pila";
    ResumeStack => "Resume Stack", "Reanudar pila";
    DeleteStack => "Delete Stack", "Eliminar pila";
    CheckStacks => "Check Stacks", "Comprobar pilas";
    SuspendStacks => "Suspend Stacks", "Suspender pilas";
    ResumeStacks => "Resume Stacks", "Reanudar pilas";
    DeleteStacks => "Delete Stacks", "Eliminar pilas";
    LaunchStack => "Launch Stack", "Lanzar pila";
    ChangeStackTemplate => "Change Stack Template", "Cambiar plantilla de la pila";
    PreviewStack => "Preview Stack", "Previsualizar pila";
    TemplateSourceLabel => "Template Source", "Origen de la plantilla";
    TemplateDirect => "Direct Input", "Entrada directa";
    TemplateData => "Template Data", "Datos de la plantilla";
    TemplateUrlHelp => "Heat downloads the template from this address.", "Heat descarga la plantilla desde esta dirección.";
    EnvironmentData => "Environment Data (optional)", "Datos del entorno (opcional)";
    EnvironmentKeep => "Leave it empty to keep the stack's environment.", "Déjalo vacío para mantener el entorno de la pila.";
    StackName => "Stack Name", "Nombre de la pila";
    StackNameHelp => "Starts with a letter; letters, digits, _, - and . only.", "Empieza por una letra; solo letras, dígitos, _, - y .";
    StackTimeoutMinutes => "Creation Timeout (minutes)", "Tiempo de espera de creación (minutos)";
    StackParamsLocal => "Dry-run: the parameters were read from the template; Heat has not validated it.", "Modo de prueba: los parámetros se han leído de la plantilla; Heat no la ha validado.";
    StackParamKeep => "Parameters left blank keep their current value.", "Los parámetros que dejes en blanco mantienen su valor actual.";
    NoTemplateParams => "The template has no parameters.", "La plantilla no tiene parámetros.";
    CommaSeparated => "Separate values with commas.", "Separa los valores con comas.";
    PreviewDryRun => "Dry-run: the preview request was built but not sent.", "Modo de prueba: la petición de previsualización se ha construido pero no se ha enviado.";
    PreviewResources => "Resources the stack would create", "Recursos que crearía la pila";
    LaunchStackIntro => "Heat will create the template's resources in this project:", "Heat creará los recursos de la plantilla en este proyecto:";
    UpdateStackIntro => "Heat will update the stack to the new template; resources the template no longer has are deleted:", "Heat actualizará la pila a la nueva plantilla; se eliminarán los recursos que la plantilla ya no tenga:";
    StackDeletesResources => "Deleting a stack also deletes every resource it created (instances, volumes, networks…).", "Eliminar una pila elimina también todos los recursos que creó (instancias, volúmenes, redes…).";
    ResourceTypes => "Resource Types", "Tipos de recursos";
    TemplateVersions => "Template Versions", "Versiones de plantilla";
    SupportStatus => "Support Status", "Estado de soporte";
    Properties => "Properties", "Propiedades";
    Attributes => "Attributes", "Atributos";
    RequiredProp => "Required", "Obligatoria";
    UpdateAllowed => "Updatable", "Actualizable";
    DefaultValue => "Default", "Predeterminado";
    Constraints => "Constraints", "Restricciones";
    Schema => "Schema (JSON)", "Esquema (JSON)";
    Version => "Version", "Versión";
    Aliases => "Aliases", "Alias";
    Function => "Function", "Función";
    Minutes => "min", "min";
    // Template Generator (roadmap 92)
    TemplateGenerator => "Template Generator", "Generador de plantillas";
    TgOtherType => "Other type…", "Otro tipo…";
    TgAddResource => "Add a resource", "Añadir un recurso";
    TgResources => "Resources", "Recursos";
    TgNoResourcesYet => "Add a resource from the palette.", "Añade un recurso desde la paleta.";
    TgPickResource => "Select a resource to edit its properties.", "Selecciona un recurso para editar sus propiedades.";
    TgNoProperties => "This type has no properties.", "Este tipo no tiene propiedades.";
    TgShowAll => "Show all properties", "Mostrar todas las propiedades";
    TgShowSet => "Show only required and set properties", "Mostrar solo las obligatorias y las que tienen valor";
    TgFromValue => "Value", "Valor";
    TgFromParam => "Parameter", "Parámetro";
    TgFromResource => "Resource", "Recurso";
    TgValueSource => "Value source", "Origen del valor";
    TgJsonHelp => "JSON, for example [{\"network\": {\"get_resource\": \"net_1\"}}].", "JSON, por ejemplo [{\"network\": {\"get_resource\": \"net_1\"}}].";
    TgDependsHelp => "Created after these, besides the resources its properties point at.", "Se crea después de estos, además de los recursos a los que apuntan sus propiedades.";
    TgNameTaken => "Another resource already has that name.", "Ya hay otro recurso con ese nombre.";
    TgAddParameter => "Add Parameter", "Añadir parámetro";
    TgAddOutput => "Add Output", "Añadir salida";
    TgAttribute => "Attribute", "Atributo";
    TgHidden => "Hidden", "Oculto";
    TgNoParameters => "No parameters: every value is in the template.", "Sin parámetros: todos los valores están en la plantilla.";
    TgNoOutputs => "No outputs.", "Sin salidas.";
    TgClear => "Clear", "Vaciar";
    TgClearConfirm => "Remove every resource, parameter and output of the draft?", "¿Quitar todos los recursos, parámetros y salidas del borrador?";
    TgDraftKept => "The draft is kept in this browser until you clear it.", "El borrador se guarda en este navegador hasta que lo vacíes.";
    TgNoProblems => "Nothing Heat would refuse was found; Launch Stack validates it with Heat.", "No se ha encontrado nada que Heat rechace; Lanzar pila la valida con Heat.";
    TgPNoResources => "A template needs at least one resource.", "Una plantilla necesita al menos un recurso.";
    TgPEmptyName => "Every parameter, resource and output needs a name.", "Cada parámetro, recurso y salida necesita un nombre.";
    TgPDuplicate => "Name used twice:", "Nombre repetido:";
    TgPMissing => "Required property not set:", "Propiedad obligatoria sin valor:";
    TgPBadValue => "Not a valid value of its type:", "No es un valor válido de su tipo:";
    TgPCycle => "Circular dependency:", "Dependencia circular:";
    TgPOutput => "Output without a resource or attribute:", "Salida sin recurso o atributo:";
    TgCanvas => "Template canvas", "Lienzo de la plantilla";
    TgCanvasHelp => "Drag a type from the palette onto the canvas, or click it. Drag a resource to move it; drag its + handle onto another resource to link them. Gray resources still lack required properties.", "Arrastra un tipo de la paleta al lienzo, o haz clic en él. Arrastra un recurso para moverlo; arrastra su asa + hasta otro recurso para enlazarlos. Los recursos en gris aún no tienen todas sus propiedades obligatorias.";
    TgLinkHandle => "Drag onto another resource to link them", "Arrastra hasta otro recurso para enlazarlos";
    TgLinkAs => "Link as:", "Enlazar como:";
    TgRemoveLink => "Remove link", "Quitar enlace";
    TgLinkFailed => "That list property is not a JSON list; fix it in the form first.", "Esa propiedad de lista no es una lista JSON; corrígela antes en el formulario.";

    // Alarming (Aodh, roadmap 79)
    Monitoring => "Monitoring", "Monitorización";
    Alarms => "Alarms", "Alarmas";
    AlarmState => "State", "Estado";
    StateSince => "State since", "Estado desde";
    Severity => "Severity", "Gravedad";
    Condition => "Condition", "Condición";
    AlarmHistory => "History", "Historial";
    AlarmRuleCard => "Rule", "Regla";
    AlarmActionsCard => "Actions on state change", "Acciones al cambiar de estado";
    Metric => "Metric", "Métrica";
    Aggregation => "Aggregation", "Agregación";
    Comparison => "Comparison", "Comparación";
    Threshold => "Threshold", "Umbral";
    Granularity => "Granularity", "Granularidad";
    EvaluationPeriods => "Evaluation periods", "Periodos de evaluación";
    Query => "Query", "Consulta";
    EventType => "Event type", "Tipo de evento";
    OnAlarm => "On alarm", "Al entrar en alarma";
    OnOk => "On OK", "Al volver a OK";
    OnInsufficientData => "On insufficient data", "Con datos insuficientes";
    RepeatActions => "Repeat actions", "Repetir acciones";
    TimeConstraints => "Time constraints", "Restricciones horarias";
    AlwaysEvaluated => "None: evaluated at all times.", "Ninguna: se evalúa siempre.";
    CronStart => "Start (cron)", "Inicio (cron)";
    Duration => "Duration", "Duración";
    Timezone => "Time zone", "Zona horaria";
    Change => "Change", "Cambio";
    Details => "Details", "Detalles";
    Seconds => "s", "s";
    StateOk => "OK", "OK";
    StateAlarm => "Alarm", "Alarma";
    StateInsufficientData => "Insufficient data", "Datos insuficientes";
    SeverityLow => "Low", "Baja";
    SeverityModerate => "Moderate", "Moderada";
    SeverityCritical => "Critical", "Crítica";
    ChangeCreation => "Created", "Creación";
    ChangeRule => "Rule change", "Cambio de regla";
    ChangeState => "State transition", "Cambio de estado";
    ChangeDeletion => "Deleted", "Eliminación";
    NoAlarmHistory => "No history.", "Sin historial.";
    MoreInfo => "More information", "Más información";
    TipAlarmType => "Where the alarm gets its data from:\n- **Prometheus query**: a PromQL expression over the cloud's telemetry; usual on *Red Hat OpenStack Services on OpenShift* (RHOSO).\n- **Gnocchi: one resource's metric**: one metric of one resource, e.g. the `cpu` of one instance.\n- **Gnocchi: aggregation of metrics**: several metrics, given by ID, combined into one value.\n- **Gnocchi: aggregation of resources**: one metric over every resource matching a search, e.g. all the instances of an autoscaling group.\n- **Event**: fires when OpenStack emits a notification, e.g. an instance is deleted; it has *no threshold*.\nUse the Gnocchi types *only* if the cloud stores its metrics in Gnocchi; ask the cloud's operators if unsure.", "De dónde obtiene la alarma sus datos:\n- **Consulta de Prometheus**: una expresión PromQL sobre la telemetría de la nube; lo habitual en *Red Hat OpenStack Services on OpenShift* (RHOSO).\n- **Gnocchi: métrica de un recurso**: una métrica de un recurso, p. ej. la `cpu` de una instancia.\n- **Gnocchi: agregación de métricas**: varias métricas, indicadas por ID, combinadas en un valor.\n- **Gnocchi: agregación de recursos**: una métrica de todos los recursos que cumplen una búsqueda, p. ej. todas las instancias de un grupo de autoescalado.\n- **Evento**: salta cuando OpenStack emite una notificación, p. ej. al eliminar una instancia; *no tiene umbral*.\nUse los tipos de Gnocchi *solo* si la nube guarda sus métricas en Gnocchi; en caso de duda, consulte a los operadores de la nube.";
    TipPromql => "A PromQL expression that returns **one number**; the alarm compares it with the threshold. Ceilometer's metrics are usually named `ceilometer_<metric>`, with the labels `resource` (the resource's ID) and `project`:\n- `ceilometer_cpu`: CPU time used, in *nanoseconds* (a counter: use `rate()`).\n- `ceilometer_memory_usage`: memory used by the instance, in *MB*.\n- `ceilometer_disk_device_read_bytes`, `ceilometer_disk_device_write_bytes`: disk I/O, in *bytes* (counters).\n- `ceilometer_network_incoming_bytes`, `ceilometer_network_outgoing_bytes`: network traffic, in *bytes* (counters).\n**Examples:**\n- CPU % of one vCPU: `rate(ceilometer_cpu{resource=\"<instance ID>\"}[5m]) / 10000000`\n- Memory in MB: `ceilometer_memory_usage{resource=\"<instance ID>\"}`\n- Average CPU % of a group: `avg(rate(ceilometer_cpu{resource=~\"<ID1>|<ID2>\"}[5m])) / 10000000`\nThe metric names available depend on the cloud; check them in its Prometheus or observability dashboard.", "Una expresión PromQL que devuelve **un número**; la alarma lo compara con el umbral. Las métricas de Ceilometer suelen llamarse `ceilometer_<métrica>`, con las etiquetas `resource` (el ID del recurso) y `project`:\n- `ceilometer_cpu`: tiempo de CPU usado, en *nanosegundos* (un contador: use `rate()`).\n- `ceilometer_memory_usage`: memoria usada por la instancia, en *MB*.\n- `ceilometer_disk_device_read_bytes`, `ceilometer_disk_device_write_bytes`: E/S de disco, en *bytes* (contadores).\n- `ceilometer_network_incoming_bytes`, `ceilometer_network_outgoing_bytes`: tráfico de red, en *bytes* (contadores).\n**Ejemplos:**\n- % de CPU de una vCPU: `rate(ceilometer_cpu{resource=\"<ID de instancia>\"}[5m]) / 10000000`\n- Memoria en MB: `ceilometer_memory_usage{resource=\"<ID de instancia>\"}`\n- % de CPU medio de un grupo: `avg(rate(ceilometer_cpu{resource=~\"<ID1>|<ID2>\"}[5m])) / 10000000`\nLas métricas disponibles dependen de la nube; compruébelas en su Prometheus o panel de observabilidad.";
    TipMetric => "The name of a Gnocchi metric of the resource. Common ones (Ceilometer):\n- `cpu`: CPU time used, in *nanoseconds*; a counter, so aggregate it with `rate:mean`.\n- `vcpus`: number of vCPUs.\n- `memory`: memory allocated to the instance, in *MB*.\n- `memory.usage`: memory used by the instance, in *MB*.\n- `disk.root.size`, `disk.ephemeral.size`: disk sizes, in *GB*.\n- `disk.device.read.bytes`, `disk.device.write.bytes`: disk I/O, in *bytes* (resource type `instance_disk`).\n- `network.incoming.bytes`, `network.outgoing.bytes`: traffic, in *bytes* (resource type `instance_network_interface`).\n- `volume.size`: a volume's size, in *GB* (resource type `volume`).\nA resource's metrics are listed by `openstack metric resource show <resource ID>`.", "El nombre de una métrica de Gnocchi del recurso. Las habituales (Ceilometer):\n- `cpu`: tiempo de CPU usado, en *nanosegundos*; es un contador, agréguelo con `rate:mean`.\n- `vcpus`: número de vCPU.\n- `memory`: memoria asignada a la instancia, en *MB*.\n- `memory.usage`: memoria usada por la instancia, en *MB*.\n- `disk.root.size`, `disk.ephemeral.size`: tamaño de los discos, en *GB*.\n- `disk.device.read.bytes`, `disk.device.write.bytes`: E/S de disco, en *bytes* (tipo de recurso `instance_disk`).\n- `network.incoming.bytes`, `network.outgoing.bytes`: tráfico, en *bytes* (tipo de recurso `instance_network_interface`).\n- `volume.size`: tamaño de un volumen, en *GB* (tipo de recurso `volume`).\nLas métricas de un recurso se listan con `openstack metric resource show <ID del recurso>`.";
    TipMetricIds => "The **IDs** (UUIDs) of the Gnocchi metrics to combine, *one per line*, e.g. the `cpu` metric of several instances. The aggregation combines them into one value per period.\nFind a metric's ID with `openstack metric resource show <resource ID>` (the *metrics* section lists name and ID).", "Los **ID** (UUID) de las métricas de Gnocchi que se combinan, *uno por línea*, p. ej. la métrica `cpu` de varias instancias. La agregación las combina en un valor por periodo.\nEl ID de una métrica se obtiene con `openstack metric resource show <ID del recurso>` (la sección *metrics* lista nombre e ID).";
    TipResourceType => "The kind of Gnocchi resource the metric belongs to:\n- `instance`: an instance (its ID is the instance's ID).\n- `instance_disk`: one disk of an instance (`disk.device.*` metrics; `cpu` is *not* here).\n- `instance_network_interface`: one network interface of an instance (`network.*` metrics).\n- `volume`: a block storage volume.\n- `image`: an image.\nList the types with `openstack metric resource-type list`.", "El tipo de recurso de Gnocchi al que pertenece la métrica:\n- `instance`: una instancia (su ID es el de la instancia).\n- `instance_disk`: un disco de una instancia (métricas `disk.device.*`; `cpu` *no* está aquí).\n- `instance_network_interface`: una interfaz de red de una instancia (métricas `network.*`).\n- `volume`: un volumen de almacenamiento de bloques.\n- `image`: una imagen.\nLos tipos se listan con `openstack metric resource-type list`.";
    TipResourceId => "The ID of the resource whose metric is watched:\n- `instance`: the instance's ID, shown on its page in **Compute › Instances**.\n- `volume`: the volume's ID, from **Volumes › Volumes**.\n- `instance_disk`, `instance_network_interface`: the Gnocchi resource ID; find it with `openstack metric resource list --type <type>`.", "El ID del recurso cuya métrica se vigila:\n- `instance`: el ID de la instancia, en su página de **Compute › Instancias**.\n- `volume`: el ID del volumen, en **Volúmenes › Volúmenes**.\n- `instance_disk`, `instance_network_interface`: el ID del recurso en Gnocchi; se obtiene con `openstack metric resource list --type <tipo>`.";
    TipGnocchiSearch => "A Gnocchi resource search, in **JSON**: the alarm aggregates the metric over every resource of the resource type that matches it. Operators: `=`, `!=`, `<`, `>`, `like` (with `%` as wildcard), `in`, `and`, `or`, `not`.\n**Examples:**\n- Instances of a Heat autoscaling group: `{\"=\": {\"server_group\": \"<stack ID>\"}}`\n- Instances whose name starts with `web-`: `{\"like\": {\"display_name\": \"web-%\"}}`\n- One flavor only: `{\"and\": [{\"=\": {\"flavor_name\": \"m1.small\"}}, {\"=\": {\"server_group\": \"web\"}}]}`\nAodh adds the project automatically, so *only this project's resources* are included.", "Una búsqueda de recursos de Gnocchi, en **JSON**: la alarma agrega la métrica de todos los recursos del tipo indicado que la cumplen. Operadores: `=`, `!=`, `<`, `>`, `like` (con `%` como comodín), `in`, `and`, `or`, `not`.\n**Ejemplos:**\n- Instancias de un grupo de autoescalado de Heat: `{\"=\": {\"server_group\": \"<ID de la pila>\"}}`\n- Instancias cuyo nombre empieza por `web-`: `{\"like\": {\"display_name\": \"web-%\"}}`\n- Solo un sabor: `{\"and\": [{\"=\": {\"flavor_name\": \"m1.small\"}}, {\"=\": {\"server_group\": \"web\"}}]}`\nAodh añade el proyecto automáticamente, así que *solo se incluyen los recursos de este proyecto*.";
    TipAggregation => "How the measures of each period are reduced to **one value**:\n- `mean`, `max`, `min`, `median`: for gauges such as `memory.usage` or `vcpus`.\n- `sum`, `count`: totals over the period.\n- `last`, `first`: the newest or oldest measure.\n- `std`: standard deviation.\n- `rate:mean`, `rate:last`: the change between measures, for *counters that only grow* such as `cpu` or `network.incoming.bytes`.\nWith several metrics or resources, the values are then averaged across them.", "Cómo se reducen las medidas de cada periodo a **un valor**:\n- `mean`, `max`, `min`, `median`: para medidores como `memory.usage` o `vcpus`.\n- `sum`, `count`: totales del periodo.\n- `last`, `first`: la medida más reciente o la más antigua.\n- `std`: desviación típica.\n- `rate:mean`, `rate:last`: la variación entre medidas, para *contadores que solo crecen* como `cpu` o `network.incoming.bytes`.\nCon varias métricas o recursos, los valores se promedian después entre ellos.";
    TipThreshold => "The value the alarm compares with, **in the metric's unit**. Examples:\n- CPU above 80 % of one vCPU, Gnocchi `cpu` with `rate:mean` and granularity `300`: 0.8 × 300 × 10⁹ ns = `240000000000`.\n- CPU above 80 % with the Prometheus example query (already in %): `80`.\n- Memory used above 2 GB (`memory.usage`, in MB): `2048`.", "El valor con el que compara la alarma, **en la unidad de la métrica**. Ejemplos:\n- CPU por encima del 80 % de una vCPU, `cpu` de Gnocchi con `rate:mean` y granularidad `300`: 0,8 × 300 × 10⁹ ns = `240000000000`.\n- CPU por encima del 80 % con la consulta de ejemplo de Prometheus (ya en %): `80`.\n- Memoria usada por encima de 2 GB (`memory.usage`, en MB): `2048`.";
    TipGranularity => "The length of each period, **in seconds**. It must be one the metric's archive policy keeps, or the alarm stays in *Insufficient data*:\n- `300` (5 minutes): Gnocchi's `low` policy, the usual default.\n- `60` (1 minute): the `medium` and `high` policies.\nCheck it with `openstack metric show <metric ID>` (*archive_policy*).\nGranularity × evaluation periods is how long the condition must last: `300` × `3` = **15 minutes**.", "La duración de cada periodo, **en segundos**. Debe ser una de las que guarda la política de archivo de la métrica; si no, la alarma se queda en *Datos insuficientes*:\n- `300` (5 minutos): la política `low` de Gnocchi, la habitual por defecto.\n- `60` (1 minuto): las políticas `medium` y `high`.\nCompruébelo con `openstack metric show <ID de la métrica>` (*archive_policy*).\nGranularidad × periodos de evaluación es cuánto debe durar la condición: `300` × `3` = **15 minutos**.";
    TipEvaluationPeriods => "How many periods **in a row** must cross the threshold before the alarm changes state. `1` reacts at once; `3` or more avoids alarms for *short peaks*. With granularity `300`, 3 periods = **15 minutes**.", "Cuántos periodos **seguidos** deben superar el umbral antes de que la alarma cambie de estado. `1` reacciona en seguida; `3` o más evita alarmas por *picos breves*. Con granularidad `300`, 3 periodos = **15 minutos**.";
    TipEventType => "The notification to react to; `*` matches any text, so `compute.instance.*` covers every instance event. Common ones:\n- `compute.instance.create.end`, `compute.instance.delete.end`: an instance was created or deleted.\n- `compute.instance.power_off.end`, `compute.instance.power_on.end`: it was shut off or started.\n- `compute.instance.update`: its state changed (filter with `traits.state`).\n- `volume.create.end`, `volume.delete.end`, `volume.attach.end`, `volume.detach.end`: volume changes.\n- `image.delete`: an image was deleted.\nThe cloud's event pipeline decides which events are stored; *ask its operators* if one never fires.", "La notificación a la que se reacciona; `*` coincide con cualquier texto, así que `compute.instance.*` cubre todos los eventos de instancias. Los habituales:\n- `compute.instance.create.end`, `compute.instance.delete.end`: se creó o eliminó una instancia.\n- `compute.instance.power_off.end`, `compute.instance.power_on.end`: se apagó o arrancó.\n- `compute.instance.update`: cambió su estado (filtre con `traits.state`).\n- `volume.create.end`, `volume.delete.end`, `volume.attach.end`, `volume.detach.end`: cambios de volúmenes.\n- `image.delete`: se eliminó una imagen.\nLa configuración de eventos de la nube decide qué eventos se guardan; *consulte a sus operadores* si uno no salta nunca.";
    TipEventQuery => "Optional conditions on the event's traits, *one per line* as `field operator value`; **all** must hold. Operators: `=` `!=` `<` `<=` `>` `>=`. Common traits:\n- `traits.instance_id = <instance ID>`: only events of this instance.\n- `traits.state = error`: an instance went to *ERROR* (with `compute.instance.update`).\n- `traits.resource_id = <ID>`: volumes and other resources.\nAodh only matches events of *this project*.", "Condiciones opcionales sobre los traits del evento, *una por línea* como `campo operador valor`; deben cumplirse **todas**. Operadores: `=` `!=` `<` `<=` `>` `>=`. Traits habituales:\n- `traits.instance_id = <ID de instancia>`: solo los eventos de esa instancia.\n- `traits.state = error`: una instancia pasó a *ERROR* (con `compute.instance.update`).\n- `traits.resource_id = <ID>`: volúmenes y otros recursos.\nAodh solo tiene en cuenta los eventos de *este proyecto*.";
    TipAlarmActions => "What Aodh calls when the alarm **enters** the state, one URL per line:\n- `https://…` or `http://…`: a **webhook**; Aodh POSTs a JSON body with the alarm's name, ID, previous and current state and reason.\n- `trust+https://…`: the same with a Keystone token of yours, for OpenStack APIs such as a **Heat scaling signal** (the `alarm_url` of an `OS::Heat::ScalingPolicy`).\n- `log://`: only writes a line in Aodh's log; *useful for testing*, but **admins only**: Aodh refuses it for other users.\n- `zaqar://?queue_name=<queue>`: posts a message to a Zaqar queue, if the cloud has Zaqar.", "Lo que llama Aodh cuando la alarma **entra** en el estado, una URL por línea:\n- `https://…` o `http://…`: un **webhook**; Aodh envía por POST un JSON con el nombre, el ID, el estado anterior y el actual de la alarma y el motivo.\n- `trust+https://…`: lo mismo con un token de Keystone suyo, para API de OpenStack como una **señal de escalado de Heat** (la `alarm_url` de un `OS::Heat::ScalingPolicy`).\n- `log://`: solo escribe una línea en el registro de Aodh; *útil para pruebas*, pero **solo para administradores**: Aodh lo rechaza para el resto de usuarios.\n- `zaqar://?queue_name=<cola>`: publica un mensaje en una cola de Zaqar, si la nube tiene Zaqar.";
    CreateAlarm => "Create Alarm", "Crear alarma";
    EditAlarm => "Edit Alarm", "Editar alarma";
    AlarmTypePrometheus => "Prometheus query", "Consulta de Prometheus";
    AlarmTypeGnocchiResource => "Gnocchi: one resource's metric", "Gnocchi: métrica de un recurso";
    AlarmTypeGnocchiMetrics => "Gnocchi: aggregation of metrics", "Gnocchi: agregación de métricas";
    AlarmTypeGnocchiResources => "Gnocchi: aggregation of resources", "Gnocchi: agregación de recursos";
    AlarmTypeEvent => "Event", "Evento";
    MetricIds => "Metric IDs", "IDs de métricas";
    GranularitySeconds => "Granularity (s)", "Granularidad (s)";
    PromqlHelp => "PromQL expression; its value is compared with the threshold.", "Expresión PromQL; su valor se compara con el umbral.";
    MetricIdsHelp => "Gnocchi metric IDs, one per line.", "IDs de métricas de Gnocchi, uno por línea.";
    AlarmResourceIdHelp => "For resource type instance, the instance's ID.", "Para el tipo de recurso instance, el ID de la instancia.";
    GnocchiSearchHelp => "Gnocchi resource search, as JSON: {\"=\": {\"server_group\": \"web\"}}.", "Búsqueda de recursos de Gnocchi, en JSON: {\"=\": {\"server_group\": \"web\"}}.";
    GranularityHelp => "Seconds; one of the metric's archive policy granularities.", "Segundos; una de las granularidades de la política de archivo de la métrica.";
    EventTypeHelp => "Notification type; * matches any, compute.instance.* a prefix.", "Tipo de notificación; * coincide con cualquiera, compute.instance.* con un prefijo.";
    EventQueryHelp => "Optional trait filters, one per line: field op value (traits.instance_id = <ID>; op: = != < <= > >=).", "Filtros de traits opcionales, uno por línea: campo op valor (traits.instance_id = <ID>; op: = != < <= > >=).";
    AlarmActionsHelp => "URLs called on entering the state, one per line: http(s)://, trust+http(s)://, zaqar://, log:// (admins only).", "URL a las que se llama al entrar en el estado, una por línea: http(s)://, trust+http(s)://, zaqar://, log:// (solo administradores).";
    AlarmNameHelp => "Shown in lists and in the notifications the alarm sends.", "Se muestra en las listas y en las notificaciones que envía la alarma.";
    AlarmDescriptionHelp => "Optional: what the alarm is for, or what to do when it fires.", "Opcional: para qué sirve la alarma o qué hacer cuando salta.";
    AlarmTypeHelp => "What the alarm evaluates: a Prometheus query, Gnocchi metrics or an event. Fixed once the alarm exists.", "Qué evalúa la alarma: una consulta de Prometheus, métricas de Gnocchi o un evento. No se puede cambiar una vez creada.";
    AlarmMetricHelp => "Gnocchi metric name, e.g. cpu or memory.usage.", "Nombre de la métrica de Gnocchi, p. ej. cpu o memory.usage.";
    AlarmResourceTypeHelp => "Gnocchi resource type, e.g. instance, volume or image.", "Tipo de recurso de Gnocchi, p. ej. instance, volume o image.";
    AggregationHelp => "How the measures of each period are combined before the comparison.", "Cómo se combinan las medidas de cada periodo antes de comparar.";
    ComparisonHelp => "The alarm fires when the value compared with the threshold this way is true.", "La alarma salta cuando la comparación del valor con el umbral se cumple.";
    ThresholdHelp => "The value to compare with; decimals allowed.", "El valor con el que se compara; admite decimales.";
    EvaluationPeriodsHelp => "Consecutive periods that must cross the threshold before the state changes.", "Periodos consecutivos que deben superar el umbral antes de cambiar de estado.";
    SeverityHelp => "How serious the alarm is; it is sent with the notifications.", "Gravedad de la alarma; se envía con las notificaciones.";
    AlarmEnabledHelp => "A disabled alarm is not evaluated and calls no actions.", "Una alarma deshabilitada no se evalúa ni llama a ninguna acción.";
    OkActionsHelp => "URLs called when the alarm returns to OK, one per line.", "URL a las que se llama cuando la alarma vuelve a OK, una por línea.";
    InsufficientDataActionsHelp => "URLs called when there is not enough data to evaluate, one per line.", "URL a las que se llama cuando no hay datos suficientes para evaluar, una por línea.";
    RepeatActionsHelp => "Call the actions on every evaluation while the alarm stays in the state, not only on entering it.", "Llamar a las acciones en cada evaluación mientras la alarma siga en el estado, no solo al entrar en él.";

    // Key Manager (Barbican, roadmap 55)
    KeyManager => "Key Manager", "Gestor de claves";
    Secrets => "Secrets", "Secretos";
    Containers => "Containers", "Contenedores";
    StoreSecret => "Store Secret", "Almacenar secreto";
    DeleteSecret => "Delete Secret", "Eliminar secreto";
    DeleteSecrets => "Delete Secrets", "Eliminar secretos";
    CreateContainer => "Create Container", "Crear contenedor";
    DeleteContainer => "Delete Container", "Eliminar contenedor";
    DeleteContainers => "Delete Containers", "Eliminar contenedores";
    SecretType => "Secret Type", "Tipo de secreto";
    ContainerType => "Container Type", "Tipo de contenedor";
    BitLength => "Bit Length", "Longitud en bits";
    CipherMode => "Mode", "Modo";
    ContentType => "Content Type", "Tipo de contenido";
    SecretRef => "Secret Reference", "Referencia del secreto";
    ContainerRef => "Container Reference", "Referencia del contenedor";
    CreatorId => "Creator", "Creador";
    Consumers => "Consumers", "Consumidores";
    NoConsumers => "No service is registered as using it.", "Ningún servicio consta como usuario.";
    ConsumersUnsupported => "This Key Manager does not report the consumers of secrets.", "Este gestor de claves no informa de los consumidores de los secretos.";
    ResourceType => "Resource Type", "Tipo de recurso";
    ResourceId => "Resource ID", "ID del recurso";
    Url => "URL", "URL";
    AccessControl => "Access Control", "Control de acceso";
    ProjectAccess => "Project members can read it", "Los miembros del proyecto pueden leerlo";
    ReadUsers => "Users with read access", "Usuarios con acceso de lectura";
    EditAccess => "Edit Access", "Editar acceso";
    ResetAccess => "Reset Access", "Restablecer acceso";
    AccessPrivate => "Private", "Privado";
    ProjectAccessField => "Every member of the project can read it", "Todos los miembros del proyecto pueden leerlo";
    AclUsersField => "Users with read access (IDs)", "Usuarios con acceso de lectura (ID)";
    AclUsersHelp => "Keystone user IDs, separated by commas or new lines; users of other projects too (e.g. a service user). An admin finds them in Identity › Users; anyone can run `openstack user show <name> -f value -c id`.", "ID de usuario de Keystone, separados por comas o saltos de línea; también de otros proyectos (p. ej. un usuario de servicio). Un administrador los encuentra en Identidad › Usuarios; cualquiera puede usar `openstack user show <nombre> -f value -c id`.";
    ResetAccessIntro => "Back to Barbican's default: every member of the project can read it, and nobody else.", "Vuelve a la configuración predeterminada de Barbican: todos los miembros del proyecto pueden leerlo, y nadie más.";
    AclUsersLoseAccess => "These users lose their access", "Estos usuarios pierden el acceso";
    AclInUse => "It is in use: see Consumers on its page.", "Está en uso: consulta Consumidores en su página.";
    UsedByLoadBalancers => "Load balancers using it (Octavia doesn't register as a Barbican consumer)", "Balanceadores que lo usan (Octavia no se registra como consumidor en Barbican)";
    ViaContainer => "Through Container", "A través del contenedor";
    AclInUseWarning => "A service such as Octavia reads it with its own user from this list: removing that user, or resetting access, breaks it the next time it reads the certificate (e.g. on a failover).", "Un servicio como Octavia lo lee con su propio usuario de esta lista: si se quita ese usuario o se restablece el acceso, fallará la próxima vez que lea el certificado (p. ej. en una conmutación por error).";
    ContainerAclSecrets => "This changes the container's access only: each secret in it has its own access list, and a load balancer needs to read both.", "Esto solo cambia el acceso al contenedor: cada secreto tiene su propia lista de acceso, y un balanceador necesita leer ambos.";
    Payload => "Payload", "Contenido";
    PayloadFormat => "Payload Format", "Formato del contenido";
    PayloadText => "Text", "Texto";
    PayloadBase64 => "Binary (base64)", "Binario (base64)";
    PayloadHelp => "Leave it empty to store only the metadata. Binary payloads are pasted as base64.", "Déjelo vacío para guardar solo los metadatos. El contenido binario se pega en base64.";
    ShowPayload => "Show Payload", "Mostrar contenido";
    HidePayload => "Hide Payload", "Ocultar contenido";
    ShowPayloadConfirm => "The secret's payload is sensitive: it will be shown in this browser, where anyone who can see the screen can read or copy it. Show it?", "El contenido del secreto es sensible: se mostrará en este navegador, donde cualquiera que vea la pantalla puede leerlo o copiarlo. ¿Mostrarlo?";
    SecretNameHelp => "Optional; unnamed secrets are listed by ID.", "Opcional; los secretos sin nombre se muestran por su ID.";
    SecretTypeHelp => "What the secret holds: Opaque (anything), Passphrase (a password), Symmetric (a key for a cipher such as AES), Certificate, Private Key or Public Key (PEM). Services check it: Octavia's TLS listeners expect a Certificate and a Private Key.", "Qué contiene el secreto: Opaco (cualquier cosa), Frase de paso (una contraseña), Simétrica (una clave para un cifrado como AES), Certificado, Clave privada o Clave pública (PEM). Los servicios lo comprueban: los listeners TLS de Octavia esperan un Certificado y una Clave privada.";
    PayloadFormatHelp => "Text for passwords and PEM files (certificates, keys). Binary for raw bytes, such as a symmetric key, pasted base64-encoded (e.g. the output of openssl rand -base64 32).", "Texto para contraseñas y ficheros PEM (certificados, claves). Binario para bytes en bruto, como una clave simétrica, pegados en base64 (p. ej. la salida de openssl rand -base64 32).";
    AlgorithmHelp => "Informational metadata Barbican stores with the secret, e.g. aes for a symmetric key or rsa for a key pair. Barbican does not check it against the payload.", "Metadato informativo que Barbican guarda con el secreto, p. ej. aes para una clave simétrica o rsa para un par de claves. Barbican no lo contrasta con el contenido.";
    BitLengthHelp => "The key's size in bits, e.g. 256 for AES-256 or 2048 for RSA. Metadata, like the algorithm.", "El tamaño de la clave en bits, p. ej. 256 para AES-256 o 2048 para RSA. Metadato, como el algoritmo.";
    CipherModeHelp => "The block cipher mode the key is meant for, e.g. cbc, gcm or ctr. Metadata; leave it empty when it doesn't apply.", "El modo de cifrado por bloques al que se destina la clave, p. ej. cbc, gcm o ctr. Metadato; déjelo vacío si no aplica.";
    SecretExpirationHelp => "In UTC. After it, Barbican refuses to return the secret. A date with no time means 00:00; a time with no date means today. Empty = never expires.", "En UTC. Pasada esa fecha, Barbican deja de devolver el secreto. Una fecha sin hora significa 00:00; una hora sin fecha significa hoy. Vacío = no caduca.";
    PayloadWarning => "The secret is shown in this browser. Hide it when done.", "El secreto se muestra en este navegador. Ocúltelo al terminar.";
    PayloadBase64Note => "Binary payload, shown base64-encoded.", "Contenido binario, mostrado en base64.";
    Expired => "Expired", "Caducado";
    ExpiresSoon => "Expires within 30 days", "Caduca en menos de 30 días";
    SecretInUse => "Services using these secrets (consumers) stop working when they are deleted.", "Los servicios que usan estos secretos (consumidores) dejan de funcionar al eliminarlos.";
    SecretConsumersOf => "In use by", "En uso por";
    ContainerKeepsSecrets => "Only the container is deleted; the secrets in it stay.", "Solo se elimina el contenedor; sus secretos se conservan.";
    ContainerSecretsHelp => "Pick a secret for each role. A certificate container (Octavia's TLS listeners) needs the certificate; an RSA one, the public and private keys.", "Elija un secreto para cada rol. Un contenedor de certificado (listeners TLS de Octavia) necesita el certificado; uno RSA, las claves pública y privada.";

    // Trunks
    Trunks => "Trunks", "Trunks";
    CreateTrunk => "Create Trunk", "Crear trunk";
    EditTrunk => "Edit Trunk", "Editar trunk";
    DeleteTrunk => "Delete Trunk", "Eliminar trunk";
    DeleteTrunks => "Delete Trunks", "Eliminar trunks";
    // Firewall (FWaaS v2, roadmap 93): neutron-fwaas-dashboard's names
    Firewall => "Firewall", "Cortafuegos";
    FirewallGroups => "Firewall Groups", "Grupos de cortafuegos";
    FirewallPolicies => "Firewall Policies", "Políticas de cortafuegos";
    FirewallRules => "Firewall Rules", "Reglas de cortafuegos";
    FirewallGroup => "Firewall Group", "Grupo de cortafuegos";
    FirewallPolicy => "Firewall Policy", "Política de cortafuegos";
    FirewallRule => "Firewall Rule", "Regla de cortafuegos";
    DeleteRules => "Delete Rules", "Eliminar reglas";
    AddPolicy => "Add Policy", "Añadir política";
    InsertRule => "Insert Rule", "Insertar regla";
    RemoveRule => "Remove Rule", "Quitar regla";
    CreateFirewallGroup => "Create Firewall Group", "Crear grupo de cortafuegos";
    EditFirewallGroup => "Edit Firewall Group", "Editar grupo de cortafuegos";
    DeleteFirewallGroup => "Delete Firewall Group", "Eliminar grupo de cortafuegos";
    DeleteFirewallGroups => "Delete Firewall Groups", "Eliminar grupos de cortafuegos";
    AddPort => "Add Port", "Añadir puerto";
    RemovePort => "Remove Port", "Quitar puerto";
    Audited => "Audited", "Auditada";
    SourceIp => "Source IP", "IP de origen";
    SourcePort => "Source Port", "Puerto de origen";
    DestinationIp => "Destination IP", "IP de destino";
    DestinationPort => "Destination Port", "Puerto de destino";
    AnyValue => "Any", "Cualquiera";
    FirewallAllow => "Allow", "Permitir";
    FirewallDeny => "Deny", "Denegar";
    FirewallReject => "Reject", "Rechazar";
    IngressPolicy => "Ingress Policy", "Política de entrada";
    EgressPolicy => "Egress Policy", "Política de salida";
    NoPolicy => "None (no filtering)", "Ninguna (sin filtrado)";
    TipIngressPolicy => "Traffic **entering the router or instance** through a port of the group, seen from that device:\n- **Instance port**: what reaches the instance, e.g. SSH or HTTP connections *to* it.\n- **Router interface**: what the subnet behind that interface sends into the router, i.e. its instances' traffic *going out* to other subnets or the Internet.\nRules are checked in order and the first match decides; traffic no rule allows is dropped. Replies to allowed connections pass.", "Tráfico que **entra en el router o la instancia** por un puerto del grupo, visto desde ese dispositivo:\n- **Puerto de instancia**: lo que llega a la instancia, p. ej. conexiones SSH o HTTP *hacia* ella.\n- **Interfaz de router**: lo que la subred de esa interfaz envía al router, es decir, el tráfico de sus instancias *hacia fuera*, a otras subredes o a Internet.\nLas reglas se comprueban en orden y decide la primera que coincide; el tráfico que ninguna regla permite se descarta. Las respuestas a conexiones permitidas pasan.";
    TipEgressPolicy => "Traffic **leaving the router or instance** through a port of the group, seen from that device:\n- **Instance port**: what the instance sends, e.g. its connections *to* the Internet or other servers.\n- **Router interface**: what the router sends into the subnet behind that interface, i.e. traffic *coming in* to its instances from other subnets or the Internet (floating IPs included).\nRules are checked in order and the first match decides; traffic no rule allows is dropped. Replies to allowed connections pass.", "Tráfico que **sale del router o la instancia** por un puerto del grupo, visto desde ese dispositivo:\n- **Puerto de instancia**: lo que envía la instancia, p. ej. sus conexiones *hacia* Internet u otros servidores.\n- **Interfaz de router**: lo que el router envía a la subred de esa interfaz, es decir, el tráfico que *entra* hacia sus instancias desde otras subredes o Internet (IP flotantes incluidas).\nLas reglas se comprueban en orden y decide la primera que coincide; el tráfico que ninguna regla permite se descarta. Las respuestas a conexiones permitidas pasan.";
    Device => "Device", "Dispositivo";
    AtTheTop => "At the top", "Al principio";
    InsertAfter => "After", "Después de";
    FirewallAddressHelp => "An IP address or a CIDR of the rule's IP version; empty means any.", "Una dirección IP o un CIDR de la versión IP de la regla; vacío significa cualquiera.";
    FirewallPortHelp => "A port (22) or a range (8000:8080); empty means any.", "Un puerto (22) o un rango (8000:8080); vacío significa cualquiera.";
    InsertRuleHelp => "Rules are evaluated in order; the first that matches decides.", "Las reglas se evalúan en orden; decide la primera que coincide.";
    FirewallPortsHelp => "Router interfaces and instance ports that no other firewall group protects.", "Interfaces de router y puertos de instancia que no protege otro grupo de cortafuegos.";
    NoRulesInPolicy => "This policy has no rules.", "Esta política no tiene reglas.";
    NoPortsInGroup => "This firewall group protects no ports.", "Este grupo de cortafuegos no protege ningún puerto.";
    FirewallInactiveHelp => "Not applied: the group needs at least one port and an ingress or egress policy.", "Sin aplicar: el grupo necesita al menos un puerto y una política de entrada o de salida.";
    FirewallDefaultGroupHelp => "Neutron's default group: only its ports can change, and only an administrator can delete it.", "Grupo por defecto de Neutron: solo pueden cambiar sus puertos, y solo un administrador puede eliminarlo.";
    ParentPort => "Parent Port", "Puerto padre";
    Subport => "Sub-port", "Subpuerto";
    Subports => "Sub-ports", "Subpuertos";
    AddSubport => "Add Sub-port", "Añadir subpuerto";
    RemoveSubport => "Remove Sub-port", "Quitar subpuerto";
    SegmentationType => "Segmentation Type", "Tipo de segmentación";
    SegmentationId => "Segmentation ID", "ID de segmentación";
    VlanHelp => "A VLAN ID, 1-4094 (none for inherit).", "Un ID de VLAN, 1-4094 (ninguno para inherit).";
    SubportOptionalHelp => "Optional: more can be added later.", "Opcional: se pueden añadir más después.";
    SubportNotParent => "A sub-port cannot be the parent port.", "Un subpuerto no puede ser el puerto padre.";

    // Network QoS
    NetworkQos => "Network QoS", "QoS de red";
    QosPolicy => "QoS Policy", "Política de QoS";
    QosPolicies => "QoS Policies", "Políticas de QoS";
    CreatePolicy => "Create Policy", "Crear política";
    EditPolicy => "Edit Policy", "Editar política";
    DeletePolicy => "Delete Policy", "Eliminar política";
    DeletePolicies => "Delete Policies", "Eliminar políticas";
    DefaultPolicy => "Default", "Predeterminada";
    BandwidthLimit => "Bandwidth Limit", "Límite de ancho de banda";
    DscpMarking => "DSCP Marking", "Marcado DSCP";
    MinimumBandwidth => "Minimum Bandwidth", "Ancho de banda mínimo";
    MinimumPacketRate => "Minimum Packet Rate", "Tasa mínima de paquetes";
    MaxKbps => "Max Bandwidth (kbps)", "Ancho de banda máx. (kbps)";
    MaxBurstKbps => "Max Burst (kb)", "Ráfaga máx. (kb)";
    DscpMark => "DSCP Mark", "Marca DSCP";
    MinKbps => "Min Bandwidth (kbps)", "Ancho de banda mín. (kbps)";
    MinKpps => "Min Packet Rate (kpps)", "Tasa mín. de paquetes (kpps)";
    AnyOnlyPacketRate => "\"any\" is only for minimum packet rate rules.", "\"any\" solo vale para reglas de tasa mínima de paquetes.";
    GatewayQosHelp => "Applied to the router's external gateway.", "Se aplica a la puerta de enlace externa del router.";
    Rule => "Rule", "Regla";

    // Load balancers (Octavia)
    LoadBalancers => "Load Balancers", "Balanceadores de carga";
    Plugins => "Enabled plugins", "Plugins habilitados";
    ReportBug => "Report a Bug", "Informar de un error";
    // Floating IP port forwarding
    PortForwarding => "Port Forwarding", "Reenvío de puertos";
    PortForwardingRules => "Port Forwarding Rules", "Reglas de reenvío de puertos";
    EditRule => "Edit Rule", "Editar regla";
    InternalIpAddress => "Internal IP Address", "Dirección IP interna";
    InternalPort => "Internal Port", "Puerto interno";
    ExternalPort => "External Port", "Puerto externo";
    PortHelp => "A port number, 1-65535.", "Un número de puerto, 1-65535.";
    PortOrRangeHelp => "A port (22) or a range (8080:8090); both sides must cover the same number of ports.", "Un puerto (22) o un rango (8080:8090); ambos lados deben cubrir el mismo número de puertos.";
    ForwardingNeedsUnassociated => "Port forwarding needs a floating IP that is not associated with a port: disassociate it first.", "El reenvío de puertos requiere una IP flotante no asociada a un puerto: desasóciala primero.";
    ShowPassword => "Show", "Mostrar";
    AdminPassword => "Admin Password", "Contraseña de administrador";
    AdminPasswordHelp => "Set by the hypervisor on first boot; leave empty to let the image decide.", "La fija el hipervisor en el primer arranque; déjala vacía para que decida la imagen.";
    HidePassword => "Hide", "Ocultar";
    DeleteLoadBalancer => "Delete Load Balancer", "Eliminar balanceador de carga";
    DeleteLoadBalancers => "Delete Load Balancers", "Eliminar balanceadores de carga";
    CreateLoadBalancer => "Create Load Balancer", "Crear balanceador de carga";
    EditLoadBalancer => "Edit Load Balancer", "Editar balanceador de carga";
    DisassociateFloatingIp => "Disassociate Floating IP", "Desasociar IP flotante";
    LoadBalancer => "Load Balancer", "Balanceador de carga";
    Listener => "Listener", "Listener";
    CreateListener => "Create Listener", "Crear listener";
    EditListener => "Edit Listener", "Editar listener";
    DeleteListener => "Delete Listener", "Eliminar listener";
    CreatePool => "Create Pool", "Crear pool";
    EditPool => "Edit Pool", "Editar pool";
    DeletePool => "Delete Pool", "Eliminar pool";
    AddMember => "Add Member", "Añadir miembro";
    EditMember => "Edit Member", "Editar miembro";
    DeleteMember => "Delete Member", "Eliminar miembro";
    CreateHealthMonitor => "Create Health Monitor", "Crear monitor de estado";
    EditHealthMonitor => "Edit Health Monitor", "Editar monitor de estado";
    DeleteHealthMonitor => "Delete Health Monitor", "Eliminar monitor de estado";
    MemberPort => "Member Port", "Puerto del miembro";
    CookieName => "Cookie Name", "Nombre de la cookie";
    LbAdvanced => "Advanced options", "Opciones avanzadas";
    LbAdvancedHelp => "Timeouts, headers and allowed networks. Empty fields keep Octavia's defaults.", "Tiempos de espera, cabeceras y redes permitidas. Los campos vacíos mantienen los valores por defecto de Octavia.";
    TimeoutClientData => "Client Data Timeout (ms)", "Tiempo de espera de datos del cliente (ms)";
    TimeoutClientDataHelp => "Frontend client inactivity timeout. Default 50000.", "Inactividad del cliente antes de cerrar la conexión. Por defecto 50000.";
    TimeoutMemberConnect => "Member Connect Timeout (ms)", "Tiempo de conexión a miembros (ms)";
    TimeoutMemberConnectHelp => "Backend member connection timeout. Default 5000.", "Tiempo máximo para conectar con un miembro. Por defecto 5000.";
    TimeoutMemberData => "Member Data Timeout (ms)", "Tiempo de espera de datos de miembros (ms)";
    TimeoutMemberDataHelp => "Backend member inactivity timeout. Default 50000.", "Inactividad de un miembro antes de cerrar la conexión. Por defecto 50000.";
    TimeoutTcpInspect => "TCP Inspect Timeout (ms)", "Tiempo de inspección TCP (ms)";
    TimeoutTcpInspectHelp => "Time to wait for more TCP packets for content inspection. Default 0.", "Tiempo de espera de más paquetes TCP para inspeccionar el contenido. Por defecto 0.";
    InsertHeaders => "Insert Headers", "Insertar cabeceras";
    InsertHeadersHelp => "Headers added to the requests sent to the members.", "Cabeceras añadidas a las peticiones enviadas a los miembros.";
    AllowedCidrs => "Allowed CIDRs", "CIDR permitidos";
    AllowedCidrsHelp => "One network per line (e.g. 192.0.2.0/24). Empty: any address.", "Una red por línea (p. ej. 192.0.2.0/24). Vacío: cualquier dirección.";
    PoolTlsEnabled => "TLS Enabled", "TLS habilitado";
    PoolTlsEnabledHelp => "Encrypt the connections to the members with TLS.", "Cifrar con TLS las conexiones con los miembros.";
    TlsCiphers => "TLS Cipher String", "Cadena de cifrados TLS";
    L7Policies => "L7 Policies", "Políticas L7";
    L7Policy => "L7 Policy", "Política L7";
    L7PoliciesHelp => "Route or reject HTTP requests by host, path, file type, header or cookie. A policy applies when all its rules match; the lowest position goes first.", "Enrutan o rechazan peticiones HTTP según host, ruta, tipo de archivo, cabecera o cookie. Una política se aplica cuando se cumplen todas sus reglas; la de menor posición va primero.";
    CreateL7Policy => "Create L7 Policy", "Crear política L7";
    EditL7Policy => "Edit L7 Policy", "Editar política L7";
    DeleteL7Policy => "Delete L7 Policy", "Eliminar política L7";
    L7Rules => "L7 Rules", "Reglas L7";
    AddL7Rule => "Add L7 Rule", "Añadir regla L7";
    EditL7Rule => "Edit L7 Rule", "Editar regla L7";
    DeleteL7Rule => "Delete L7 Rule", "Eliminar regla L7";
    Position => "Position", "Posición";
    L7PositionHelp => "1 is evaluated first. Empty: last.", "La 1 se evalúa primero. Vacío: la última.";
    RedirectPool => "Redirect Pool", "Pool de redirección";
    RedirectUrl => "Redirect URL", "URL de redirección";
    L7Target => "Target", "Destino";
    RuleType => "Type", "Tipo";
    CompareType => "Compare Type", "Tipo de comparación";
    L7Key => "Key", "Clave";
    L7KeyHelp => "The header or cookie name.", "El nombre de la cabecera o de la cookie.";
    Invert => "Invert", "Invertir";
    InvertHelp => "Match when the comparison fails.", "Coincide cuando la comparación no se cumple.";
    L7NeedsHttpListener => "L7 policies need an HTTP or TERMINATED_HTTPS listener.", "Las políticas L7 necesitan un listener HTTP o TERMINATED_HTTPS.";
    LbCertificate => "Certificate", "Certificado";
    LbCertificateHelp => "From Key Manager: certificate containers with a private key, and PKCS#12 bundles. Octavia ends HTTPS with it.", "Del gestor de claves: contenedores de certificado con clave privada y paquetes PKCS#12. Octavia termina HTTPS con él.";
    SniCertificates => "SNI Certificates", "Certificados SNI";
    SniCertificatesHelp => "More certificates, chosen by the host name the client asks for.", "Más certificados, elegidos según el nombre de host que pide el cliente.";
    TlsCiphersHelp => "OpenSSL cipher list, colon-separated. Empty: the cloud's default.", "Lista de cifrados de OpenSSL separada por dos puntos. Vacío: el valor por defecto de la nube.";
    HttpMethod => "HTTP Method", "Método HTTP";
    UrlPath => "URL Path", "Ruta URL";
    ExpectedCodes => "Expected Codes", "Códigos esperados";
    ExpectedCodesHelp => "A code (200), a list (200,202) or a range (200-204).", "Un código (200), una lista (200,202) o un rango (200-204).";
    TimeoutHelp => "Not longer than the delay.", "No mayor que el intervalo.";
    UnlimitedWhenEmpty => "Empty = unlimited.", "Vacío = ilimitado.";
    VipAddressHelp => "Empty = an address of the subnet is chosen automatically.", "Vacío = se elige automáticamente una dirección de la subred.";
    PoolListenerHelp => "Optional: becomes the default pool of a listener that has none.", "Opcional: pasa a ser el pool predeterminado de un listener que no tiene.";
    InvalidSettings => "Check the values", "Revisa los valores";
    LbDetailsHelp => "The load balancer gets a virtual IP (VIP) on this subnet; associate a floating IP to reach it from outside.", "El balanceador obtiene una IP virtual (VIP) en esta subred; asocia una IP flotante para llegar a ella desde fuera.";
    ListenerHelp => "What the VIP accepts: protocol and port. Optional.", "Lo que acepta la VIP: protocolo y puerto. Opcional.";
    PoolHelp => "Where the listener sends traffic, and how it spreads it.", "Adónde envía el tráfico el listener y cómo lo reparte.";
    MembersHelp => "The addresses that receive the traffic, each with its own port and weight: instance addresses, or any address on a subnet the load balancer reaches.", "Las direcciones que reciben el tráfico, cada una con su puerto y peso: direcciones de instancia, o cualquier dirección en una subred que alcance el balanceador.";
    MonitorAddress => "Monitor Address", "Dirección de monitorización";
    MonitorPort => "Monitor Port", "Puerto de monitorización";
    MonitorAddressHelp => "Health-check this address instead of the member's. Empty = the member itself.", "Comprobar esta dirección en lugar de la del miembro. Vacío = el propio miembro.";
    NoMembers => "No members.", "Sin miembros.";
    AllocatedMembers => "Allocated Members", "Miembros asignados";
    AvailableInstances => "Available Instances", "Instancias disponibles";
    AddExternalMember => "Add external member", "Añadir miembro externo";
    AddAction => "Add", "Añadir";
    AdminStateUp => "Admin State Up", "Estado de administración activo";
    MembersRemoved => "Saving deletes these members:", "Al guardar se eliminan estos miembros:";
    EditMembers => "Edit Members", "Editar miembros";
    EditMembersIntro => "The pool's members become exactly this list: new rows are added, changed ones updated, and removed ones deleted.", "Los miembros del grupo pasan a ser exactamente esta lista: las filas nuevas se añaden, las cambiadas se actualizan y las quitadas se eliminan.";
    MemberSource => "Address", "Dirección";
    ExternalAddress => "External address", "Dirección externa";
    MonitorHelp => "How the pool checks that each member is healthy.", "Cómo comprueba el pool que cada miembro está sano.";
    LbCascade => "Its listeners, pools, members and health monitors are deleted with it.", "Sus listeners, pools, miembros y monitores de estado se eliminan con él.";
    OperatingStatus => "Operating Status", "Estado operativo";
    ProvisioningStatus => "Provisioning Status", "Estado de aprovisionamiento";
    Provider => "Provider", "Proveedor";
    Vip => "Virtual IP (VIP)", "IP virtual (VIP)";
    Listeners => "Listeners", "Listeners";
    Pools => "Pools", "Pools";
    HealthMonitors => "Health Monitors", "Monitores de estado";
    HealthMonitor => "Health Monitor", "Monitor de estado";
    Protocol => "Protocol", "Protocolo";
    DefaultPool => "Default Pool", "Pool predeterminado";
    ConnectionLimit => "Connection Limit", "Límite de conexiones";
    Unlimited => "Unlimited", "Ilimitado";
    Algorithm => "Algorithm", "Algoritmo";
    SessionPersistence => "Session Persistence", "Persistencia de sesión";
    Weight => "Weight", "Peso";
    Backup => "Backup", "Respaldo";
    Backups => "Volume Backups", "Copias de seguridad de volúmenes";
    CreateBackup => "Create Volume Backup", "Crear copia de seguridad de volumen";
    EditBackup => "Edit Backup", "Editar copia de seguridad";
    RestoreBackup => "Restore Backup", "Restaurar copia de seguridad";
    DeleteBackup => "Delete Volume Backup", "Eliminar copia de seguridad de volumen";
    DeleteBackups => "Delete Volume Backups", "Eliminar copias de seguridad de volumen";
    BackupName => "Backup Name", "Nombre de la copia";
    IncrementalBackup => "Incremental", "Incremental";
    IncrementalBackupHelp => "Only the changes since the volume's last backup. Needs an earlier full backup of the same volume.", "Solo los cambios desde la última copia del volumen. Requiere una copia completa anterior del mismo volumen.";
    ForceBackup => "Force (back up an attached volume)", "Forzar (copia de un volumen adjunto)";
    RestoreToVolume => "Restore to Volume", "Restaurar en el volumen";
    RestoreToVolumeHelp => "Leave empty to create a new volume. Choosing a volume overwrites its data.", "Déjelo vacío para crear un volumen nuevo. Si elige un volumen, se sobrescriben sus datos.";
    RestoredVolumeName => "New Volume Name", "Nombre del volumen nuevo";
    FailReason => "Failure Reason", "Motivo del fallo";
    BackupContainer => "Container", "Contenedor";
    DelaySeconds => "Delay (s)", "Intervalo (s)";
    TimeoutSeconds => "Timeout (s)", "Tiempo de espera (s)";
    MaxRetries => "Max Retries", "Reintentos máximos";
    MaxRetriesDown => "Max Retries Down", "Reintentos hasta caída";
    HttpCheck => "HTTP Check", "Comprobación HTTP";

    // Routers
    CreateRouter => "Create Router", "Crear router";
    EditRouter => "Edit Router", "Editar router";
    DeleteRouter => "Delete Router", "Eliminar router";
    SetGateway => "Set Gateway", "Establecer puerta de enlace";
    ClearGateway => "Clear Gateway", "Quitar puerta de enlace";
    AddInterface => "Add Interface", "Añadir interfaz";
    DeleteInterface => "Delete Interface", "Eliminar interfaz";
    StaticRoutes => "Static Routes", "Rutas estáticas";
    Destination => "Destination CIDR", "CIDR de destino";
    NextHop => "Next Hop", "Siguiente salto";
    AddStaticRoute => "Add Static Route", "Añadir ruta estática";
    DeleteStaticRoute => "Delete Static Route", "Eliminar ruta estática";
    NextHopHelp => "An address inside one of the router's subnets.", "Una dirección dentro de una de las subredes del router.";
    Snat => "SNAT", "SNAT";
    ExternalFixedIps => "External Fixed IPs", "IP fijas externas";

    // Security groups
    CreateSecurityGroup => "Create Security Group", "Crear grupo de seguridad";
    EditSecurityGroup => "Edit Security Group", "Editar grupo de seguridad";
    DeleteSecurityGroup => "Delete Security Group", "Eliminar grupo de seguridad";
    ManageRules => "Manage Rules", "Gestionar reglas";
    AddRule => "Add Rule", "Añadir regla";
    DeleteRule => "Delete Rule", "Eliminar regla";
    Direction => "Direction", "Dirección";
    Ingress => "Ingress", "Entrada";
    Egress => "Egress", "Salida";
    EtherType => "Ether Type", "Tipo Ether";
    IpProtocol => "IP Protocol", "Protocolo IP";
    PortRange => "Port Range", "Rango de puertos";
    PortFrom => "From Port", "Puerto inicial";
    PortTo => "To Port", "Puerto final";
    RemoteCidr => "Remote CIDR", "CIDR remoto";
    RemoteGroup => "Remote Security Group", "Grupo de seguridad remoto";
    // *Add Rule* help under each field (roadmap 61).
    DirectionHelp => "Ingress: traffic arriving at the instances. Egress: traffic leaving them.", "Entrada: tráfico que llega a las instancias. Salida: tráfico que sale de ellas.";
    EtherTypeHelp => "IPv4 or IPv6; it must match the family of the remote CIDR.", "IPv4 o IPv6; debe coincidir con la familia del CIDR remoto.";
    IpProtocolHelp => "Empty: any protocol (then ports don't apply). Otherwise TCP, UDP or ICMP.", "Vacío: cualquier protocolo (los puertos no se aplican). Si no, TCP, UDP o ICMP.";
    PortFromHelp => "A port (1-65535), or the first of a range. Empty: every port. For ICMP, the ICMP type (8 = echo request, ping).", "Un puerto (1-65535) o el primero de un rango. Vacío: todos los puertos. En ICMP, el tipo ICMP (8 = solicitud de eco, ping).";
    PortToHelp => "The last port of the range. Empty: the same as From Port. For ICMP, the ICMP code.", "El último puerto del rango. Vacío: el mismo que el puerto inicial. En ICMP, el código ICMP.";
    RemoteCidrHelp => "The addresses the rule allows: 0.0.0.0/0 is anywhere in IPv4, ::/0 anywhere in IPv6.", "Las direcciones que permite la regla: 0.0.0.0/0 es cualquiera en IPv4, ::/0 cualquiera en IPv6.";
    RemoteGroupHelp => "Instead of a CIDR, allow traffic from instances in this group. When set, the CIDR is ignored.", "En lugar de un CIDR, permite el tráfico de las instancias de este grupo. Si se indica, el CIDR se ignora.";
    RuleDescriptionHelp => "Optional; shown in the rules table.", "Opcional; se muestra en la tabla de reglas.";
    Remote => "Remote", "Remoto";

    // Floating IPs
    AllocateIp => "Allocate IP To Project", "Asignar IP al proyecto";
    ReleaseIp => "Release Floating IP", "Liberar IP flotante";
    Associate => "Associate", "Asociar";
    Disassociate => "Disassociate", "Desasociar";
    PortToAssociate => "Port to be associated", "Puerto que se asociará";

    // Quick wins (roadmap 1, 6, 8, 9)
    Unlocked => "Unlocked", "Desbloqueada";
    FromVolume => "from volume", "desde volumen";
    Ephemeral => "Ephemeral Disk", "Disco efímero";
    Swap => "Swap", "Swap";
    RootDisk => "Root Disk", "Disco raíz";
    FlavorDetails => "Flavor details", "Detalles del sabor";
    Copy => "Copy", "Copiar";
    Copied => "Copied", "Copiado";
    Updating => "Updating…", "Actualizando…";

    // API access
    Service => "Service", "Servicio";
    ServiceEndpoint => "Service Endpoint", "Endpoint del servicio";

    // Error page (roadmap 14)
    BackToSafety => "Back to safety", "Volver a un lugar seguro";
    Retry => "Retry", "Reintentar";
    TechnicalDetails => "Technical details", "Detalles técnicos";
    ErrorKindLabel => "Error type", "Tipo de error";
    ErrUnauthTitle => "Your session has expired", "Tu sesión ha caducado";
    ErrUnauthText => "Sign in again to continue. Keystone sessions last about one hour.", "Vuelve a iniciar sesión para continuar. Las sesiones de Keystone duran aproximadamente una hora.";
    ErrForbiddenTitle => "Access denied", "Acceso denegado";
    ErrForbiddenText => "Your user does not have permission to see this in the current project.", "Tu usuario no tiene permiso para ver esto en el proyecto actual.";
    ErrNotFoundTitle => "Not found", "No encontrado";
    ErrNotFoundText => "This resource does not exist, or it was deleted. It may also belong to another project or region.", "Este recurso no existe o se ha eliminado. También puede pertenecer a otro proyecto o región.";
    ErrServiceTitle => "Service not available in this region", "Servicio no disponible en esta región";
    ErrServiceText => "The current region does not offer the service this page needs. Try another region.", "La región actual no ofrece el servicio que necesita esta página. Prueba con otra región.";
    ErrRegionTitle => "Region not available", "Región no disponible";
    ErrRegionText => "This region is not in your catalog any more. Choose another one.", "Esta región ya no está en tu catálogo. Elige otra.";
    ErrTimeoutTitle => "The cloud did not respond", "La nube no ha respondido";
    ErrTimeoutText => "OpenStack took too long to answer or could not be reached. It is usually temporary.", "OpenStack ha tardado demasiado en responder o no se ha podido contactar. Suele ser algo temporal.";
    ErrOpenStackTitle => "OpenStack returned an error", "OpenStack ha devuelto un error";
    ErrOpenStackText => "The request reached OpenStack, but it could not complete it.", "La petición ha llegado a OpenStack, pero no ha podido completarla.";
    ErrInvalidTitle => "Invalid request", "Petición no válida";
    ErrInvalidText => "Moonlight rejected the request before sending it to OpenStack.", "Moonlight ha rechazado la petición antes de enviarla a OpenStack.";
    ErrInternalTitle => "Something went wrong", "Algo ha salido mal";
    ErrTooManyTitle => "Too many attempts", "Demasiados intentos";
    LoginThrottled => "Too many failed sign-ins. Wait a few minutes and try again.", "Demasiados inicios de sesión fallidos. Espera unos minutos y vuelve a intentarlo.";
    ErrInternalText => "An unexpected error occurred while loading this page.", "Se ha producido un error inesperado al cargar esta página.";
    RegionFallback => "The region you were using is not available any more. Moonlight switched to", "La región que usabas ya no está disponible. Moonlight ha cambiado a";

    // Theme (roadmap 18)
    Theme => "Theme", "Tema";
    ThemeLight => "Light", "Claro";
    ThemeDark => "Dark", "Oscuro";
    ThemeSystem => "Auto", "Automático";

    // Network Topology (roadmap 20)
    NetworkTopology => "Network Topology", "Topología de red";
    TopologyTab => "Topology", "Topología";
    GraphTab => "Graph", "Grafo";
    SizeSmall => "Small", "Pequeño";
    SizeNormal => "Normal", "Normal";
    ToggleLabels => "Toggle Labels", "Mostrar u ocultar etiquetas";
    ToggleCollapse => "Toggle Network Collapse", "Contraer o expandir redes";
    CenterTopology => "Center Topology", "Centrar topología";
    ListView => "List", "Lista";
    ComputingLayout => "Computing layout…", "Calculando la disposición…";
    TopologyEmpty => "This project has no networks, routers or instances.", "Este proyecto no tiene redes, routers ni instancias.";
    TopologyHint => "Drag to pan, scroll to zoom. Click a node to open it.", "Arrastra para desplazar y usa la rueda para ampliar. Haz clic en un nodo para abrirlo.";
    Connections => "Connections", "Conexiones";
    Router => "Router", "Router";
    Gateway => "gateway", "puerta de enlace";
    // Instance actions parity (roadmap 21)
    AssociateFloatingIp => "Associate Floating IP", "Asociar IP flotante";
    FloatingIp => "Floating IP", "IP flotante";
    AllocateNewIp => "Allocate a new IP from a pool", "Asignar una IP nueva de un pool";
    AttachInterface => "Attach Interface", "Adjuntar interfaz";
    DetachInterface => "Detach Interface", "Desadjuntar interfaz";
    InterfaceSource => "The way to specify an interface", "Forma de indicar la interfaz";
    ByNetwork => "by Network (and IP address)", "por red (y dirección IP)";
    ByPort => "by Port", "por puerto";
    FixedIpAddress => "Fixed IP Address", "Dirección IP fija";
    FixedIpHelp => "Leave empty to let OpenStack choose one.", "Déjalo vacío para que OpenStack elija una.";
    Port => "Port", "Puerto";
    EditPortSecurityGroups => "Edit Port Security Groups", "Editar grupos de seguridad del puerto";
    EditSecurityGroups => "Edit Security Groups", "Editar grupos de seguridad";
    EditSecurityGroupsHelp => "Applied to every port of the instance. To change a single port, use Edit Port Security Groups.", "Se aplican a todos los puertos de la instancia. Para cambiar un solo puerto, usa Editar grupos de seguridad del puerto.";
    ResizeInstance => "Resize Instance", "Redimensionar instancia";
    NewFlavor => "New Flavor", "Sabor nuevo";
    SameFlavor => "Choose a flavor different from the current one.", "Elige un sabor distinto del actual.";
    ConfirmResize => "Confirm Resize/Migrate", "Confirmar redimensión/migración";
    RevertResize => "Revert Resize/Migrate", "Revertir redimensión/migración";
    RebuildInstance => "Rebuild Instance", "Reconstruir instancia";
    RebuildHelp => "The instance is rebuilt from this image; data on its root disk is lost.", "La instancia se reconstruye desde esta imagen; se pierden los datos de su disco raíz.";
    ShelveInstance => "Shelve Instance", "Archivar instancia";
    UnshelveInstance => "Unshelve Instance", "Desarchivar instancia";
    AnyAvailabilityZone => "Any Availability Zone", "Cualquier zona de disponibilidad";
    AnyZone => "Leave empty to keep the instance's zone.", "Déjalo vacío para mantener la zona de la instancia.";
    RescueInstance => "Rescue Instance", "Rescatar instancia";
    UnrescueInstance => "Unrescue Instance", "Salir del rescate";
    RescueImage => "Rescue image", "Imagen de rescate";
    RescueImageHelp => "Optional. By default the instance's own image is used.", "Opcional. Por defecto se usa la imagen de la propia instancia.";
    PasswordTitle => "Save the rescue password now", "Guarda ahora la contraseña de rescate";
    LaunchFromRow => "Launch", "Lanzar";
    LaunchAsInstance => "Launch as Instance", "Lanzar como instancia";
    StartInstances => "Start Instances", "Iniciar instancias";
    ShutOffInstances => "Shut Off Instances", "Apagar instancias";
    SoftRebootInstances => "Soft Reboot Instances", "Reinicio suave de instancias";
    // Volume actions parity (roadmap 22)
    ChangeVolumeType => "Change Volume Type", "Cambiar tipo de volumen";
    NewVolumeType => "Type", "Tipo";
    MigrateOnDemand => "Allow migration to another backend", "Permitir la migración a otro backend";
    MigrateHelp => "Needed when the new type lives on a different backend. The data is copied, which can take long.", "Necesario si el tipo nuevo está en otro backend. Se copian los datos, lo que puede tardar.";
    SameVolumeType => "Choose a type different from the current one.", "Elige un tipo distinto del actual.";
    UploadToImage => "Upload to Image", "Subir a imagen";
    ForceUpload => "Force (the volume is attached)", "Forzar (el volumen está adjunto)";
    CreateTransfer => "Create Transfer", "Crear transferencia";
    TransferName => "Transfer Name", "Nombre de la transferencia";
    TransferHelp => "The receiving project needs the transfer ID and the authorization key you get next.", "El proyecto receptor necesita el ID de transferencia y la clave de autorización que obtendrás a continuación.";
    AcceptTransfer => "Accept Transfer", "Aceptar transferencia";
    TransferId => "Transfer ID", "ID de transferencia";
    AuthKey => "Authorization Key", "Clave de autorización";
    TransferTitle => "Save the transfer ID and authorization key now", "Guarda ahora el ID de transferencia y la clave de autorización";
    AppCredTitle => "Your application credential", "Su credencial de aplicación";
    AppCredExplain => "Capture the ID and secret now, or download them: the secret will not be available after closing this dialog. If you lose it, create a new application credential.", "Copie ahora el ID y el secreto, o descárguelos: el secreto no estará disponible al cerrar este diálogo. Si lo pierde, cree una credencial de aplicación nueva.";
    Secret => "Secret", "Secreto";
    SecretHelp => "Empty = Keystone generates one.", "Vacío = Keystone genera uno.";
    CreateAppCredential => "Create Application Credential", "Crear credencial de aplicación";
    DeleteAppCredential => "Delete Application Credential", "Eliminar credencial de aplicación";
    DeleteAppCredentials => "Delete Application Credentials", "Eliminar credenciales de aplicación";
    ExpirationDate => "Expiration Date", "Fecha de caducidad";
    ExpirationTime => "Expiration Time", "Hora de caducidad";
    ExpirationHelp => "In UTC. A date with no time means 00:00; a time with no date means today. Empty = never expires.", "En UTC. Una fecha sin hora significa 00:00; una hora sin fecha significa hoy. Vacío = no caduca.";
    AppCredRolesHelp => "Empty = all of your roles in the current project.", "Vacío = todos sus roles en el proyecto actual.";
    UnrestrictedDangerous => "Unrestricted (dangerous)", "Sin restricciones (peligroso)";
    UnrestrictedHelp => "Lets the credential create and delete other application credentials and trusts. Leave it off unless your application needs it.", "Permite a la credencial crear y eliminar otras credenciales de aplicación y confianzas. Déjelo desactivado salvo que su aplicación lo necesite.";
    PickDate => "Pick a date", "Elegir una fecha";
    PickTime => "Pick a time", "Elegir una hora";
    Now => "Now", "Ahora";
    NowUtcHelp => "Fill in the current time, in UTC", "Rellenar la hora actual, en UTC";
    PreviousMonth => "Previous month", "Mes anterior";
    NextMonth => "Next month", "Mes siguiente";
    Today => "Today", "Hoy";
    Clear => "Clear", "Borrar";
    Hour => "Hour", "Hora";
    Minute => "Minute", "Minuto";
    AccessRules => "Access Rules", "Reglas de acceso";
    AccessRulesHelp => "For finer-grained delegation, a JSON or YAML list of rules, each with a service type, an HTTP method and a URL path; the credential can then call only those. Empty = no restriction. For example:", "Para una delegación más fina, una lista JSON o YAML de reglas, cada una con un tipo de servicio, un método HTTP y una ruta URL; la credencial solo podrá hacer esas llamadas. Vacío = sin restricción. Por ejemplo:";
    AppCredHelp => "An application credential lets an application authenticate as you, with your roles in the current project (or some of them), without your password.", "Una credencial de aplicación permite a una aplicación autenticarse como usted, con sus roles en el proyecto actual (o parte de ellos), sin su contraseña.";
    VolumeMessages => "Messages", "Mensajes";
    MessageLevel => "Level", "Nivel";
    // Admin › System Information (roadmap 85)
    SystemInformation => "System Information", "Información del sistema";
    CatalogServices => "Services", "Servicios";
    ComputeServices => "Compute Services", "Servicios de cómputo";
    BlockStorageServices => "Block Storage Services", "Servicios de almacenamiento de bloques";
    NetworkAgents => "Network Agents", "Agentes de red";
    Zone => "Zone", "Zona";
    ServiceState => "State", "Situación";
    LastUpdated => "Last Updated", "Última actualización";
    Endpoints => "Endpoints", "Endpoints";
    Disabled => "Disabled", "Deshabilitado";
    QuotaDefaults => "Defaults", "Valores predeterminados";
    // Admin › Compute, Volume, Network (roadmap 85)
    VolumeAdminGroup => "Volume", "Volumen";
    Hypervisors => "Hypervisors", "Hipervisores";
    HypervisorTab => "Hypervisor", "Hipervisor";
    ComputeHost => "Compute Host", "Host de cómputo";
    ResourceProvider => "Resource Provider", "Proveedor de recursos";
    Vgpus => "vGPUs", "vGPU";
    VgpuType => "vGPU Type", "Tipo de vGPU";
    VgpuUsage => "vGPU Usage", "Uso de vGPU";
    OtherResources => "Other Resources", "Otros recursos";
    ToggleDevices => "Show or hide this host's devices", "Mostrar u ocultar los dispositivos de este host";
    HypervisorSummary => "Hypervisor Summary", "Resumen de hipervisores";
    VcpuUsage => "VCPU Usage", "Uso de VCPU";
    MemoryUsage => "Memory Usage", "Uso de memoria";
    LocalDiskUsage => "Local Disk Usage", "Uso de disco local";
    ReservedRatio => "reserved", "reservado";
    Hostname => "Hostname", "Nombre de host";
    VcpusUsed => "VCPUs (used)", "VCPUs (usadas)";
    VcpusTotal => "VCPUs (total)", "VCPUs (total)";
    RamUsed => "RAM (used)", "RAM (usada)";
    RamTotal => "RAM (total)", "RAM (total)";
    DiskUsed => "Local Storage (used)", "Almacenamiento local (usado)";
    DiskTotal => "Local Storage (total)", "Almacenamiento local (total)";
    HostAggregates => "Host Aggregates", "Agregados de hosts";
    AvailabilityZones => "Availability Zones", "Zonas de disponibilidad";
    AvailabilityZoneName => "Availability Zone Name", "Nombre de la zona de disponibilidad";
    Hosts => "Hosts", "Hosts";
    Flavors => "Flavors", "Sabores";
    FlavorName => "Flavor Name", "Nombre del sabor";
    SwapDisk => "Swap Disk", "Disco de intercambio";
    RxtxFactor => "RX/TX factor", "Factor RX/TX";
    GroupTypes => "Group Types", "Tipos de grupo";
    QosSpecs => "QoS Specs", "Especificaciones de QoS";
    AssociatedQosSpec => "Associated QoS Spec", "Especificación de QoS asociada";
    Encryption => "Encryption", "Cifrado";
    Consumer => "Consumer", "Consumidor";
    RbacPolicies => "RBAC Policies", "Políticas RBAC";
    ObjectType => "Object Type", "Tipo de objeto";
    RbacObject => "Object", "Objeto";
    TargetProject => "Target Project", "Proyecto destino";
    CreateRbacPolicy => "Create RBAC Policy", "Crear política RBAC";
    AllocateFloatingIp => "Allocate Floating IP", "Asignar IP flotante";
    FloatingIpAddressOptional => "Floating IP Address (optional)", "Dirección IP flotante (opcional)";
    FloatingIpAddressHelp => "A free address of the pool; empty lets Neutron pick one.", "Una dirección libre del pool; vacío deja que Neutron elija una.";
    UpdateRbacPolicy => "Update RBAC Policy", "Actualizar política RBAC";
    SaveChanges => "Save Changes", "Guardar cambios";
    DeleteRbacPolicy => "Delete RBAC Policy", "Eliminar política RBAC";
    DeleteRbacPolicies => "Delete RBAC Policies", "Eliminar políticas RBAC";
    ActionObjectType => "Action and Object Type", "Acción y tipo de objeto";
    SharedNetwork => "Shared Network", "Red compartida";
    SharedQosPolicy => "Shared QoS Policy", "Política de QoS compartida";
    RbacTargetHelp => "The project the object is shared with; * shares it with every project.", "El proyecto con el que se comparte el objeto; * lo comparte con todos los proyectos.";
    AllProjects => "All projects", "Todos los proyectos";
    ComputeQuotas => "Compute Quotas", "Cuotas de cómputo";
    VolumeQuotas => "Volume Quotas", "Cuotas de volúmenes";
    NetworkQuotas => "Network Quotas", "Cuotas de red";
    QuotaName => "Quota Name", "Nombre de la cuota";
    MetadataDefinitions => "Metadata Definitions", "Definiciones de metadatos";
    // Modules (roadmap 19)
    Modules => "Detected capabilities", "Capacidades detectadas";
    ModulesUnknown => "Not detected yet: every panel and action is shown.", "Aún sin detectar: se muestran todos los paneles y acciones.";
    ComputeApi => "Compute API (Nova)", "API de cómputo (Nova)";
    VolumeApi => "Volume API (Cinder)", "API de volúmenes (Cinder)";
    LoadBalancerApi => "Load Balancer API (Octavia)", "API de balanceadores (Octavia)";
    HiddenPanels => "Hidden panels", "Paneles ocultos";
    NetworkExtensions => "Network extensions", "Extensiones de red";
    // Bulk actions (roadmap 10)
    SelectRow => "Select", "Seleccionar";
    SelectAllRows => "Select all visible rows", "Seleccionar todas las filas visibles";
    DeleteInstances => "Delete Instances", "Eliminar instancias";
    DeleteImages => "Delete Images", "Eliminar imágenes";
    DeleteKeyPairs => "Delete Key Pairs", "Eliminar pares de claves";
    DeleteServerGroups => "Delete Server Groups", "Eliminar grupos de servidores";
    DeleteVolumes => "Delete Volumes", "Eliminar volúmenes";
    DeleteSnapshots => "Delete Volume Snapshots", "Eliminar instantáneas de volumen";
    DeleteNetworks => "Delete Networks", "Eliminar redes";
    DeleteRouters => "Delete Routers", "Eliminar routers";
    DeleteSecurityGroups => "Delete Security Groups", "Eliminar grupos de seguridad";
    ReleaseIps => "Release Floating IPs", "Liberar IP flotantes";
    SomeItemsFailed => "Some items failed. The others were accepted.", "Algunos elementos han fallado. El resto se ha aceptado.";
    TooManySelected => "Select at most 100 items at once.", "Selecciona como máximo 100 elementos a la vez.";
    AllItemsFailed => "No item was accepted.", "No se ha aceptado ningún elemento.";
}

/// The text of `k` in `lang` (non-reactive; inside views use [`I18n::t`]).
pub fn t(lang: Lang, k: K) -> &'static str {
    match lang {
        Lang::EnUs => en(k),
        Lang::EsEs => es(k),
    }
}

/// "Displaying 3 items" / "Mostrando 3 elementos".
/// "Items 101–200" (with "of 322" when the API counted them) for a page of a paged
/// list, `from` being the 1-based number of its first row.
/// "Items 51–100", or "Items 51–100 of 346" with a total; `(n, true)` is a count
/// that stopped early (*Count all*'s cap) and reads "of 100000+".
pub fn page_range(lang: Lang, from: usize, shown: usize, count: Option<(u64, bool)>) -> String {
    let to = from + shown.saturating_sub(1);
    let range = if shown == 0 { "0".to_string() } else { format!("{from}–{to}") };
    let count = count.map(|(n, more)| if more { format!("{n}+") } else { n.to_string() });
    match (lang, count) {
        (Lang::EnUs, None) => format!("Items {range}"),
        (Lang::EnUs, Some(n)) => format!("Items {range} of {n}"),
        (Lang::EsEs, None) => format!("Elementos {range}"),
        (Lang::EsEs, Some(n)) => format!("Elementos {range} de {n}"),
    }
}

pub fn displaying(lang: Lang, n: usize) -> String {
    match (lang, n) {
        (Lang::EnUs, 1) => "Displaying 1 item".into(),
        (Lang::EnUs, n) => format!("Displaying {n} items"),
        (Lang::EsEs, 1) => "Mostrando 1 elemento".into(),
        (Lang::EsEs, n) => format!("Mostrando {n} elementos"),
    }
}

/// Bulk button label with the selection size: "Delete Volumes (3)".
pub fn with_count(lang: Lang, k: K, n: usize) -> String {
    if n == 0 { t(lang, k).to_string() } else { format!("{} ({n})", t(lang, k)) }
}

/// Quota sub-label under each overview chart.
pub fn used_of(lang: Lang, used: &str, max: Option<&str>) -> String {
    match (lang, max) {
        (Lang::EnUs, Some(m)) => format!("Used {used} of {m}"),
        (Lang::EnUs, None) => format!("Used {used} (No Limit)"),
        (Lang::EsEs, Some(m)) => format!("Usado {used} de {m}"),
        (Lang::EsEs, None) => format!("Usado {used} (sin límite)"),
    }
}

/// Human age in the style of Horizon's `timesince`: the two largest units.
pub fn age(lang: Lang, seconds: i64) -> String {
    age_parts(lang, seconds).join(", ")
}

/// [`age`] as its units (`["3 semanas", "2 días"]`), one or two, never empty: a
/// table cell renders each unbreakable so a narrow column wraps between them
/// (roadmap 72).
pub fn age_parts(lang: Lang, seconds: i64) -> Vec<String> {
    const UNITS: [(i64, &str, &str, &str, &str); 6] = [
        (365 * 86_400, "year", "years", "año", "años"),
        (30 * 86_400, "month", "months", "mes", "meses"),
        (7 * 86_400, "week", "weeks", "semana", "semanas"),
        (86_400, "day", "days", "día", "días"),
        (3_600, "hour", "hours", "hora", "horas"),
        (60, "minute", "minutes", "minuto", "minutos"),
    ];
    let mut rest = seconds.max(0);
    let mut parts = Vec::new();
    for (size, en1, enn, es1, esn) in UNITS {
        if parts.len() == 2 {
            break;
        }
        let n = rest / size;
        if n > 0 {
            rest -= n * size;
            let word = match (lang, n) {
                (Lang::EnUs, 1) => en1,
                (Lang::EnUs, _) => enn,
                (Lang::EsEs, 1) => es1,
                (Lang::EsEs, _) => esn,
            };
            parts.push(format!("{n} {word}"));
        } else if !parts.is_empty() {
            // Only adjacent units, like Horizon ("1 month, 2 days" never skips weeks).
            break;
        }
    }
    if parts.is_empty() {
        parts.push(match lang {
            Lang::EnUs => "0 minutes".into(),
            Lang::EsEs => "0 minutos".into(),
        });
    }
    parts
}

/// Localised label for a Nova server status (`ACTIVE`, `SHUTOFF`…).
pub fn server_status(lang: Lang, status: &str) -> String {
    let (en, es) = match status.to_ascii_uppercase().as_str() {
        "ACTIVE" => ("Active", "Activa"),
        "SHUTOFF" => ("Shutoff", "Apagada"),
        "ERROR" => ("Error", "Error"),
        "BUILD" => ("Build", "Construyendo"),
        "PAUSED" => ("Paused", "Pausada"),
        "SUSPENDED" => ("Suspended", "Suspendida"),
        "SHELVED" | "SHELVED_OFFLOADED" => ("Shelved", "Archivada"),
        "REBOOT" | "HARD_REBOOT" => ("Rebooting", "Reiniciando"),
        "RESIZE" | "VERIFY_RESIZE" => ("Resizing", "Redimensionando"),
        "MIGRATING" => ("Migrating", "Migrando"),
        "RESCUE" => ("Rescue", "Rescate"),
        "DELETED" | "SOFT_DELETED" => ("Deleted", "Eliminada"),
        _ => return title_case(status),
    };
    match lang {
        Lang::EnUs => en.into(),
        Lang::EsEs => es.into(),
    }
}

/// Localised label for Nova's numeric power state.
/// Glance visibility as Horizon shows it (*Public*); unknown values as is.
pub fn image_visibility(lang: Lang, v: &str) -> String {
    let k = match v {
        "private" => K::VisibilityPrivate,
        "shared" => K::VisibilityShared,
        "community" => K::VisibilityCommunity,
        "public" => K::VisibilityPublic,
        other => return other.to_string(),
    };
    t(lang, k).to_string()
}

/// Nova server group policy as Horizon shows it (*Soft Anti Affinity*).
/// A month's name (1-12) for the date picker.
pub fn month_name(lang: Lang, m: u32) -> &'static str {
    const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const ES: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
    let i = (m.clamp(1, 12) - 1) as usize;
    if lang == Lang::EsEs { ES[i] } else { EN[i] }
}

/// Monday-first weekday initials for the date picker.
pub fn weekday_initials(lang: Lang) -> &'static [&'static str; 7] {
    if lang == Lang::EsEs { &["L", "M", "X", "J", "V", "S", "D"] } else { &["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] }
}

pub fn server_group_policy(lang: Lang, p: &str) -> String {
    let k = match p {
        "affinity" => K::PolicyAffinity,
        "anti-affinity" => K::PolicyAntiAffinity,
        "soft-affinity" => K::PolicySoftAffinity,
        "soft-anti-affinity" => K::PolicySoftAntiAffinity,
        other => return other.to_string(),
    };
    t(lang, k).to_string()
}

/// Nova's numeric `OS-EXT-STS:power_state`, as Horizon words it.
pub fn power_state(lang: Lang, state: i64) -> &'static str {
    match (lang, state) {
        (Lang::EnUs, 1) => "Running",
        (Lang::EnUs, 3) => "Paused",
        (Lang::EnUs, 4) => "Shut Down",
        (Lang::EnUs, 6) => "Crashed",
        (Lang::EnUs, 7) => "Suspended",
        (Lang::EnUs, _) => "No State",
        (Lang::EsEs, 1) => "En ejecución",
        (Lang::EsEs, 3) => "Pausada",
        (Lang::EsEs, 4) => "Apagada",
        (Lang::EsEs, 6) => "Bloqueada",
        (Lang::EsEs, 7) => "Suspendida",
        (Lang::EsEs, _) => "Sin estado",
    }
}

/// Localised Nova `task_state` (`spawning`, `rebooting`…).
pub fn task_state(lang: Lang, task: &str) -> String {
    let (en, es) = match task.to_ascii_lowercase().as_str() {
        "scheduling" => ("Scheduling", "Programando"),
        "block_device_mapping" => ("Block Device Mapping", "Asignando discos"),
        "networking" => ("Networking", "Configurando red"),
        "spawning" => ("Spawning", "Creando"),
        "rebooting" | "reboot_pending" | "reboot_started" => ("Rebooting", "Reiniciando"),
        "rebooting_hard" | "reboot_pending_hard" | "reboot_started_hard" => ("Hard Rebooting", "Reinicio forzado"),
        "deleting" => ("Deleting", "Eliminando"),
        "image_snapshot" | "image_snapshot_pending" | "image_pending_upload" => ("Snapshotting", "Creando instantánea"),
        "image_uploading" => ("Uploading Image", "Subiendo imagen"),
        "powering_on" => ("Powering On", "Encendiendo"),
        "powering_off" => ("Powering Off", "Apagando"),
        "pausing" => ("Pausing", "Pausando"),
        "unpausing" => ("Resuming", "Reanudando"),
        "suspending" => ("Suspending", "Suspendiendo"),
        "resuming" => ("Resuming", "Reanudando"),
        "resize_prep" | "resize_migrating" | "resize_migrated" | "resize_finish" => ("Resizing", "Redimensionando"),
        "migrating" => ("Migrating", "Migrando"),
        "rebuilding" | "rebuild_spawning" | "rebuild_block_device_mapping" => ("Rebuilding", "Reconstruyendo"),
        "shelving" | "shelving_image_uploading" | "shelving_offloading" => ("Shelving", "Archivando"),
        "unshelving" => ("Unshelving", "Desarchivando"),
        _ => return title_case(task),
    };
    match lang {
        Lang::EnUs => en.into(),
        Lang::EsEs => es.into(),
    }
}

/// Generic resource status (volumes, networks, routers, floating IPs…).
pub fn resource_status(lang: Lang, status: &str) -> String {
    let (en, es) = match status.to_ascii_lowercase().as_str() {
        "active" => ("Active", "Activo"),
        "available" => ("Available", "Disponible"),
        "in-use" => ("In-use", "En uso"),
        "down" => ("Down", "Inactivo"),
        // A FWaaS firewall group that isn't applied: no ports, or no policy (roadmap 93).
        "inactive" => ("Inactive", "Sin aplicar"),
        "error" => ("Error", "Error"),
        "creating" => ("Creating", "Creando"),
        "deleting" => ("Deleting", "Eliminando"),
        "queued" => ("Queued", "En cola"),
        "saving" => ("Saving", "Guardando"),
        "deactivated" => ("Deactivated", "Desactivada"),
        "build" => ("Build", "Construyendo"),
        // Octavia operating / provisioning statuses
        "online" => ("Online", "En línea"),
        "offline" => ("Offline", "Fuera de línea"),
        "degraded" => ("Degraded", "Degradado"),
        "draining" => ("Draining", "Drenando"),
        "no_monitor" => ("No Monitor", "Sin monitor"),
        "pending_create" => ("Pending Create", "Creación pendiente"),
        "pending_update" => ("Pending Update", "Actualización pendiente"),
        "pending_delete" => ("Pending Delete", "Eliminación pendiente"),
        _ => return title_case(status),
    };
    match lang {
        Lang::EnUs => en.into(),
        Lang::EsEs => es.into(),
    }
}

/// A Heat status (`CREATE_COMPLETE`, `UPDATE_IN_PROGRESS`…, roadmap 62):
/// action and state translated apart, as Horizon shows them ("Create Complete").
pub fn stack_status(lang: Lang, status: &str) -> String {
    let Some((action, state)) = ["_IN_PROGRESS", "_COMPLETE", "_FAILED"].iter().find_map(|st| status.strip_suffix(st).map(|a| (a, &st[1..]))) else {
        return title_case(status);
    };
    // Spanish: a noun for the action, agreeing with the state ("Creación completada").
    let (en, es, feminine) = match action {
        "CREATE" => ("Create", "Creación", true),
        "UPDATE" => ("Update", "Actualización", true),
        "DELETE" => ("Delete", "Eliminación", true),
        "ROLLBACK" => ("Rollback", "Reversión", true),
        "SUSPEND" => ("Suspend", "Suspensión", true),
        "RESUME" => ("Resume", "Reanudación", true),
        "CHECK" => ("Check", "Comprobación", true),
        "SNAPSHOT" => ("Snapshot", "Instantánea", true),
        "RESTORE" => ("Restore", "Restauración", true),
        "ADOPT" => ("Adopt", "Adopción", true),
        "INIT" => ("Init", "Inicialización", true),
        "ABANDON" => ("Abandon", "Abandono", false),
        _ => return title_case(status),
    };
    match (lang, state) {
        (Lang::EnUs, "IN_PROGRESS") => format!("{en} In Progress"),
        (Lang::EnUs, "COMPLETE") => format!("{en} Complete"),
        (Lang::EnUs, _) => format!("{en} Failed"),
        (Lang::EsEs, "IN_PROGRESS") => format!("{es} en curso"),
        (Lang::EsEs, "COMPLETE") => format!("{es} {}", if feminine { "completada" } else { "completado" }),
        (Lang::EsEs, _) => format!("{es} {}", if feminine { "fallida" } else { "fallido" }),
    }
}

/// Barbican's secret and container types and container roles (roadmap 55),
/// humanised; unknown ones title-cased.
pub fn key_manager_term(lang: Lang, term: &str) -> String {
    let (en, es) = match term {
        "symmetric" => ("Symmetric", "Simétrica"),
        "public" => ("Public Key", "Clave pública"),
        "private" => ("Private Key", "Clave privada"),
        "passphrase" => ("Passphrase", "Frase de paso"),
        "certificate" => ("Certificate", "Certificado"),
        "opaque" => ("Opaque", "Opaco"),
        "generic" => ("Generic", "Genérico"),
        "rsa" => ("RSA", "RSA"),
        "private_key" => ("Private Key", "Clave privada"),
        "public_key" => ("Public Key", "Clave pública"),
        "private_key_passphrase" => ("Private Key Passphrase", "Frase de paso de la clave privada"),
        "intermediates" => ("Intermediates", "Intermedios"),
        _ => return title_case(term),
    };
    match lang {
        Lang::EnUs => en.into(),
        Lang::EsEs => es.into(),
    }
}

fn title_case(s: &str) -> String {
    let lower = s.to_lowercase().replace('_', " ");
    let mut c = lower.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "-".into(),
    }
}

/// Formats a size given in bytes: `1.5 GB`, `512 MB`…
/// Number with `places` decimals; `Lang::EsEs` uses a decimal comma.
pub fn decimal(lang: Lang, v: f64, places: usize) -> String {
    // Never `-0.00` (roadmap 60).
    let s = crate::model::fixed(v, places);
    match lang {
        Lang::EnUs => s,
        Lang::EsEs => s.replace('.', ","),
    }
}

/// [`decimal`] with the integer part grouped by thousands: `1,234,567.89` in
/// English, `1.234.567,89` in Spanish (roadmap 86). Spanish leaves four-digit
/// numbers ungrouped (`1234,50`), as the RAE and CLDR's `es` (minimum grouping
/// digits 2) do. For display only: CSVs keep [`crate::model::fixed`].
pub fn grouped(lang: Lang, v: f64, places: usize) -> String {
    let s = crate::model::fixed(v, places);
    let (sign, rest) = s.strip_prefix('-').map_or(("", s.as_str()), |r| ("-", r));
    let (int, frac) = rest.split_once('.').map_or((rest, None), |(i, f)| (i, Some(f)));
    let (sep, point) = match lang {
        Lang::EnUs => (',', '.'),
        Lang::EsEs => ('.', ','),
    };
    let min_digits = if lang == Lang::EsEs { 5 } else { 4 };
    let mut out = String::from(sign);
    if int.len() < min_digits {
        out.push_str(int);
    } else {
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                out.push(sep);
            }
            out.push(c);
        }
    }
    if let Some(f) = frac {
        out.push(point);
        out.push_str(f);
    }
    out
}

/// A background job's progress (roadmap 87): `12 / 40`, grouped like [`grouped`].
pub fn job_count(lang: Lang, done: usize, total: usize) -> String {
    format!("{} / {}", grouped(lang, done as f64, 0), grouped(lang, total as f64, 0))
}

/// A finished background job: `38 done, 2 failed` (the failures only when any).
pub fn job_summary(lang: Lang, ok: usize, failed: usize) -> String {
    match (lang, failed) {
        (Lang::EnUs, 0) => format!("{ok} done"),
        (Lang::EnUs, f) => format!("{ok} done, {f} failed"),
        (Lang::EsEs, 0) => format!("{ok} hechos"),
        (Lang::EsEs, f) => format!("{ok} hechos, {f} con error"),
    }
}

/// A background job stopped by an expired session.
pub fn job_expired(lang: Lang, done: usize, total: usize) -> String {
    match lang {
        Lang::EnUs => format!("Stopped after {done} of {total}: the session expired. Sign in again; the rest was not sent."),
        Lang::EsEs => format!("Detenido tras {done} de {total}: la sesión ha caducado. Inicie sesión de nuevo; el resto no se ha enviado."),
    }
}

/// Launch wizard validation messages, like Horizon's.
pub fn volume_min(lang: Lang, gb: u32) -> String {
    match lang {
        Lang::EnUs => format!("The volume size must be at least {gb} GB"),
        Lang::EsEs => format!("El tamaño del volumen debe ser de al menos {gb} GB"),
    }
}

/// The password-expiry banner (`[session] password_expiry_warning_days`).
pub fn password_expires(lang: Lang, days: i64) -> String {
    match (lang, days) {
        (Lang::EnUs, 0) => "Your password expires today.".into(),
        (Lang::EnUs, 1) => "Your password expires in 1 day.".into(),
        (Lang::EnUs, d) => format!("Your password expires in {d} days."),
        (Lang::EsEs, 0) => "Tu contraseña caduca hoy.".into(),
        (Lang::EsEs, 1) => "Tu contraseña caduca en 1 día.".into(),
        (Lang::EsEs, d) => format!("Tu contraseña caduca en {d} días."),
    }
}

/// Why a CIDR breaks `[network] allowed_private_subnet_cidr`.
pub fn subnet_issue(lang: Lang, issue: &crate::prefs::SubnetIssue) -> String {
    use crate::prefs::SubnetIssue::*;
    match (lang, issue) {
        (Lang::EnUs, NotACidr) => "Not a valid network address (CIDR), e.g. 192.168.0.0/24".into(),
        (Lang::EsEs, NotACidr) => "No es una dirección de red (CIDR) válida, p. ej. 192.168.0.0/24".into(),
        (Lang::EnUs, Ipv6Disabled) => "IPv6 subnets are not allowed here".into(),
        (Lang::EsEs, Ipv6Disabled) => "Aquí no se permiten subredes IPv6".into(),
        (Lang::EnUs, Outside(r)) => format!("Must be inside one of: {}", r.join(", ")),
        (Lang::EsEs, Outside(r)) => format!("Debe estar dentro de alguno de: {}", r.join(", ")),
    }
}

/// A network's provider attributes, read-only in Admin › *Edit Network*
/// (`vlan · physnet1 · 120`); parts the type lacks are left out.
pub fn provider_summary(lang: Lang, p: &crate::model::ProviderNetwork) -> String {
    let mut parts = vec![p.network_type.clone()];
    if let Some(phys) = &p.physical_network {
        parts.push(match lang {
            Lang::EnUs => format!("physical network {phys}"),
            Lang::EsEs => format!("red física {phys}"),
        });
    }
    if let Some(id) = p.segmentation_id {
        parts.push(match lang {
            Lang::EnUs => format!("segmentation ID {id}"),
            Lang::EsEs => format!("ID de segmentación {id}"),
        });
    }
    let parts = parts.join(" · ");
    match lang {
        Lang::EnUs => format!("Provider network: {parts}. Set when the network was created; Neutron doesn't change it."),
        Lang::EsEs => format!("Red del proveedor: {parts}. Se fija al crear la red; Neutron no la cambia."),
    }
}

/// Under the vGPU donut: how many vGPU devices (Placement providers with
/// `VGPU`: physical GPUs, or their SR-IOV functions on newer cards), on how many hosts.
pub fn gpus_on_hosts(lang: Lang, gpus: usize, hosts: usize) -> String {
    let (g, h) = (if gpus == 1 { "" } else { "s" }, if hosts == 1 { "" } else { "s" });
    match lang {
        Lang::EnUs => format!("{gpus} vGPU device{g} (PCI) on {hosts} host{h}"),
        Lang::EsEs => format!("{gpus} dispositivo{g} vGPU (PCI) en {hosts} host{h}"),
    }
}

/// Why a flavor can't take the chosen image: too little RAM.
pub fn flavor_needs_ram(lang: Lang, mb: i64) -> String {
    match lang {
        Lang::EnUs => format!("The selected source requires a flavor with at least {mb} MB of RAM"),
        Lang::EsEs => format!("El origen elegido requiere un sabor con al menos {mb} MB de RAM"),
    }
}

/// Why a flavor can't take the chosen image or volume: too little disk.
pub fn flavor_needs_disk(lang: Lang, gb: i64) -> String {
    match lang {
        Lang::EnUs => format!("The selected source requires a flavor with at least {gb} GB of root disk"),
        Lang::EsEs => format!("El origen elegido requiere un sabor con al menos {gb} GB de disco raíz"),
    }
}

/// A byte count in binary units (`1.5 GB`), for image sizes.
pub fn bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v.fract() < 0.05 || i == 0 {
        format!("{:.0} {}", v, U[i])
    } else {
        format!("{:.1} {}", v, U[i])
    }
}

// ---------------------------------------------------------------------------
// Reactive glue
// ---------------------------------------------------------------------------

/// Reactive handle to the current language, provided as context by `App`.
#[derive(Clone, Copy)]
pub struct I18n(pub RwSignal<Lang>);

impl I18n {
    /// Translates `k`. Reactive when called inside a closure in a view.
    pub fn t(&self, k: K) -> &'static str {
        t(self.0.get(), k)
    }

    /// The current language (reactive inside a closure).
    pub fn lang(&self) -> Lang {
        self.0.get()
    }

    /// Switches language instantly (no reload) and remembers it in a cookie.
    pub fn set(&self, lang: Lang) {
        self.0.set(lang);
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            if let Some(doc) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok())
            {
                let _ = doc.set_cookie(&format!(
                    "{LANG_COOKIE}={}; Path=/; Max-Age=31536000; SameSite=Lax",
                    lang.cookie_value()
                ));
                if let Some(html) = doc.document_element() {
                    let _ = html.set_attribute("lang", lang.tag());
                }
            }
        }
    }
}

/// The language handle provided by the app.
pub fn use_i18n() -> I18n {
    expect_context::<I18n>()
}

/// Language for the first render: the cookie if present, else `Accept-Language`.
/// Server and client compute the same value, so hydration matches.
pub fn initial_lang() -> Lang {
    #[cfg(feature = "ssr")]
    {
        if let Some(parts) = use_context::<http::request::Parts>() {
            let cookie_lang = parts
                .headers
                .get_all(http::header::COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .find_map(|c| cookie_value(c, LANG_COOKIE).and_then(|v| Lang::parse(&v)));
            if let Some(l) = cookie_lang {
                return l;
            }
            // `[ui] default_language` beats the browser's preference.
            if let Some(l) = crate::server::config::state().config.default_language {
                return l;
            }
            if let Some(al) = parts
                .headers
                .get(http::header::ACCEPT_LANGUAGE)
                .and_then(|v| v.to_str().ok())
            {
                return Lang::from_accept_language(al);
            }
        }
        Lang::default()
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        use wasm_bindgen::JsCast;
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok());
        if let Some(doc) = doc {
            if let Some(l) = doc
                .cookie()
                .ok()
                .and_then(|c| cookie_value(&c, LANG_COOKIE))
                .and_then(|v| Lang::parse(&v))
            {
                return l;
            }
            // Fall back to what the server rendered.
            if let Some(l) = doc
                .document_element()
                .and_then(|e| e.get_attribute("lang"))
                .and_then(|v| Lang::parse(&v))
            {
                return l;
            }
        }
        Lang::default()
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        Lang::default()
    }
}

/// Extracts one cookie's value from a `Cookie` header string.
pub fn cookie_value(header: &str, name: &str) -> Option<String> {
    header.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        (k == name).then(|| v.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_range_numbers_rows_from_one_and_shows_the_total_when_known() {
        assert_eq!(page_range(Lang::EnUs, 101, 100, None), "Items 101–200");
        assert_eq!(page_range(Lang::EnUs, 1, 86, Some((186, false))), "Items 1–86 of 186");
        assert_eq!(page_range(Lang::EsEs, 1, 3, Some((3, false))), "Elementos 1–3 de 3");
        assert_eq!(page_range(Lang::EnUs, 1, 0, None), "Items 0");
        assert_eq!(page_range(Lang::EnUs, 1, 50, Some((100_000, true))), "Items 1–50 of 100000+", "a capped count says so");
    }

    #[test]
    fn stack_statuses_split_action_and_state() {
        assert_eq!(stack_status(Lang::EnUs, "CREATE_COMPLETE"), "Create Complete");
        assert_eq!(stack_status(Lang::EnUs, "UPDATE_IN_PROGRESS"), "Update In Progress");
        assert_eq!(stack_status(Lang::EsEs, "ROLLBACK_FAILED"), "Reversión fallida");
        assert_eq!(stack_status(Lang::EsEs, "ABANDON_COMPLETE"), "Abandono completado", "masculine noun");
        assert_eq!(stack_status(Lang::EnUs, "WEIRD_STATE"), "Weird state", "unknown: title case");
    }

    #[test]
    fn visibility_and_policies_read_like_horizon() {
        assert_eq!(image_visibility(Lang::EnUs, "public"), "Public");
        assert_eq!(image_visibility(Lang::EsEs, "shared"), "Compartida");
        assert_eq!(image_visibility(Lang::EnUs, "odd"), "odd");
        assert_eq!(server_group_policy(Lang::EnUs, "soft-anti-affinity"), "Soft Anti Affinity");
        assert_eq!(server_group_policy(Lang::EnUs, "custom"), "custom");
    }

    #[test]
    fn every_key_is_translated_in_both_languages() {
        for k in K::ALL {
            assert!(!t(Lang::EnUs, *k).is_empty(), "{k:?} empty in en-US");
            assert!(!t(Lang::EsEs, *k).is_empty(), "{k:?} empty in es-ES");
        }
    }

    #[test]
    fn navigation_is_actually_translated() {
        assert_eq!(t(Lang::EnUs, K::Instances), "Instances");
        assert_eq!(t(Lang::EsEs, K::Instances), "Instancias");
        assert_eq!(t(Lang::EsEs, K::Overview), "Visión general");
    }

    #[test]
    fn parses_language_tags() {
        assert_eq!(Lang::parse("es"), Some(Lang::EsEs));
        assert_eq!(Lang::parse("es-ES"), Some(Lang::EsEs));
        assert_eq!(Lang::parse("EN_us"), Some(Lang::EnUs));
        assert_eq!(Lang::parse("fr"), None);
        for l in Lang::ALL {
            assert_eq!(Lang::parse(l.cookie_value()), Some(l));
            assert_eq!(Lang::parse(l.tag()), Some(l));
        }
    }

    #[test]
    fn accept_language_respects_order_and_falls_back_to_english() {
        assert_eq!(Lang::from_accept_language("es-ES,es;q=0.9,en;q=0.8"), Lang::EsEs);
        assert_eq!(Lang::from_accept_language("fr-FR,fr;q=0.9,es;q=0.5"), Lang::EsEs);
        assert_eq!(Lang::from_accept_language("en-GB,en;q=0.9"), Lang::EnUs);
        assert_eq!(Lang::from_accept_language("de-DE"), Lang::EnUs);
        assert_eq!(Lang::from_accept_language(""), Lang::EnUs);
    }

    #[test]
    fn decimal_uses_a_comma_in_spanish() {
        assert_eq!(decimal(Lang::EnUs, 1234.567, 2), "1234.57");
        assert_eq!(decimal(Lang::EsEs, 0.5, 2), "0,50");
        // An empty `f64` sum is `-0.0` (roadmap 60): zero usage must not read `-0.00`.
        assert_eq!(decimal(Lang::EnUs, -0.0, 2), "0.00");
        assert_eq!(decimal(Lang::EsEs, -0.0, 2), "0,00");
        assert_eq!(decimal(Lang::EnUs, -0.001, 2), "0.00", "rounds to zero");
        assert_eq!(decimal(Lang::EnUs, -1.5, 2), "-1.50", "a real negative keeps its sign");
        assert_eq!(decimal(Lang::EnUs, -0.0, 0), "0");
    }

    #[test]
    fn job_texts_name_the_counts() {
        assert_eq!(job_count(Lang::EsEs, 1200, 12000), "1200 / 12.000");
        assert_eq!(job_summary(Lang::EnUs, 38, 2), "38 done, 2 failed");
        assert_eq!(job_summary(Lang::EnUs, 40, 0), "40 done");
        assert!(job_expired(Lang::EsEs, 5, 40).starts_with("Detenido tras 5 de 40"));
    }

    #[test]
    fn grouped_splits_thousands_by_language() {
        let both = |v: f64, p: usize| (grouped(Lang::EnUs, v, p), grouped(Lang::EsEs, v, p));
        assert_eq!(both(1234567.891, 2), ("1,234,567.89".into(), "1.234.567,89".into()));
        assert_eq!(both(1234.5, 2), ("1,234.50".into(), "1234,50".into()), "Spanish leaves four digits ungrouped");
        assert_eq!(both(12345.5, 2), ("12,345.50".into(), "12.345,50".into()));
        assert_eq!(both(999.99, 2), ("999.99".into(), "999,99".into()));
        assert_eq!(both(-1234567.0, 0), ("-1,234,567".into(), "-1.234.567".into()), "the sign stays in front");
        assert_eq!(both(-0.0, 2), ("0.00".into(), "0,00".into()), "never -0.00 (roadmap 60)");
        assert_eq!(both(100000.0, 0), ("100,000".into(), "100.000".into()));
    }

    #[test]
    fn age_uses_two_adjacent_units() {
        let day = 86_400;
        assert_eq!(age(Lang::EnUs, 30), "0 minutes");
        assert_eq!(age(Lang::EnUs, 3 * 60), "3 minutes");
        assert_eq!(age(Lang::EnUs, 2 * 3600 + 60), "2 hours, 1 minute");
        assert_eq!(age(Lang::EnUs, 40 * day), "1 month, 1 week");
        assert_eq!(age(Lang::EsEs, 40 * day), "1 mes, 1 semana");
        assert_eq!(age(Lang::EsEs, 2 * 365 * day), "2 años");
        // 1 month and 2 days: weeks are skipped, so stop after months
        assert_eq!(age(Lang::EnUs, 32 * day), "1 month");
        assert_eq!(age(Lang::EnUs, -5), "0 minutes");
    }

    #[test]
    fn age_parts_are_the_units_that_age_joins() {
        let day = 86_400;
        assert_eq!(age_parts(Lang::EsEs, 23 * day), ["3 semanas", "2 días"]);
        assert_eq!(age_parts(Lang::EnUs, day + 3600), ["1 day", "1 hour"]);
        assert_eq!(age_parts(Lang::EsEs, 7 * day), ["1 semana"]);
        assert_eq!(age_parts(Lang::EsEs, 0), ["0 minutos"], "never empty");
        for secs in [0, 59, 3600 + 60, 23 * day, 400 * day] {
            assert_eq!(age(Lang::EsEs, secs), age_parts(Lang::EsEs, secs).join(", "));
        }
    }

    #[test]
    fn statuses_are_localised_with_a_sane_fallback() {
        assert_eq!(server_status(Lang::EsEs, "SHUTOFF"), "Apagada");
        assert_eq!(server_status(Lang::EnUs, "active"), "Active");
        assert_eq!(server_status(Lang::EnUs, "SOME_NEW_STATE"), "Some new state");
        assert_eq!(power_state(Lang::EnUs, 1), "Running");
        assert_eq!(power_state(Lang::EsEs, 4), "Apagada");
        assert_eq!(resource_status(Lang::EsEs, "in-use"), "En uso");
    }

    #[test]
    fn task_states_are_localised() {
        assert_eq!(task_state(Lang::EsEs, "spawning"), "Creando");
        assert_eq!(task_state(Lang::EnUs, "image_snapshot_pending"), "Snapshotting");
        assert_eq!(task_state(Lang::EnUs, "new_task"), "New task");
    }

    #[test]
    fn formats_sizes_and_counters() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(10 * 1024 * 1024 * 1024), "10 GB");
        assert_eq!(displaying(Lang::EnUs, 1), "Displaying 1 item");
        assert_eq!(displaying(Lang::EsEs, 12), "Mostrando 12 elementos");
        assert_eq!(used_of(Lang::EnUs, "3", None), "Used 3 (No Limit)");
        assert_eq!(used_of(Lang::EsEs, "3", Some("10")), "Usado 3 de 10");
    }

    #[test]
    fn reads_cookie_values() {
        assert_eq!(cookie_value("a=1; ml_lang=es; b=2", LANG_COOKIE).as_deref(), Some("es"));
        assert_eq!(cookie_value("a=1", LANG_COOKIE), None);
    }
}
