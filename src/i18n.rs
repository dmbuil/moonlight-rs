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

/// Builds the [`K`] enum and its English / Spanish table from one `Key => "en", "es";` list,
/// so a key can't exist in one language and not the other.
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
    PrefixesHelp => "Per resource type, what goes before each property's name there, e.g. hw:.", "Por tipo de recurso, lo que va delante del nombre de cada propiedad en él, p. ej. hw:.";
    TipPrefixes => "When a namespace's properties apply to a resource type, its prefix goes before each property's name. For CPU pinning's `cpu_policy`:\n- `OS::Nova::Flavor` = `hw:` gives the extra spec `hw:cpu_policy`;\n- `OS::Glance::Image` = `hw_` gives the image property `hw_cpu_policy`.\nKey: the resource type; value: its prefix, empty for none. A new prefix removes the association and adds it back.", "Cuando las propiedades de un espacio de nombres se aplican a un tipo de recurso, su prefijo va delante del nombre de cada propiedad. Para `cpu_policy` de la fijación de CPU:\n- `OS::Nova::Flavor` = `hw:` da la especificación extra `hw:cpu_policy`;\n- `OS::Glance::Image` = `hw_` da la propiedad de imagen `hw_cpu_policy`.\nClave: el tipo de recurso; valor: su prefijo, vacío si no tiene. Un prefijo nuevo quita la asociación y la vuelve a añadir.";
    PrefixNotAssociated => "A prefix for a resource type that isn't selected", "Un prefijo para un tipo de recurso no seleccionado";
    FilterFirstNote => "Give a filter to list the rows: this list waits for one before asking the cloud.", "Indica un filtro para ver las filas: esta lista espera uno antes de consultar la nube.";
    DeletedSuffix => "(Deleted)", "(Eliminado)";
    ShowDeleted => "Show deleted", "Mostrar eliminadas";
    ShowDeletedHelp => "Also list the deleted instances Nova still keeps in its database (until an operator archives them), to see what an instance was. They have no actions and no detail page.", "Lista también las instancias eliminadas que Nova aún guarda en su base de datos (hasta que un operador las archive), para ver qué era una instancia. No tienen acciones ni página de detalle.";
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
    CreateUserIntro => "Create a user in this domain and, optionally, give them a role on a primary project.", "Cree un usuario en este dominio y, si quiere, dele un rol en un proyecto principal.";
    ChangePasswordIntro => "Set a new password for the user, who signs in with it from now on. Choose a strong one.", "Fije una contraseña nueva para el usuario, que entrará con ella a partir de ahora. Elija una robusta.";
    FloatingIpIntro => "A floating IP is a fixed public address from a pool (an external network): associate it with an instance, and move it between instances, to reach them from outside the cloud.", "Una IP flotante es una dirección pública fija de un pool (una red externa): asóciela a una instancia, y muévala entre instancias, para llegar a ellas desde fuera de la nube.";
    SecurityGroupIntro => "A security group is a set of IP filter rules applied to the network interfaces of instances. It starts with rules that allow all outgoing traffic; add the incoming ones next.", "Un grupo de seguridad es un conjunto de reglas de filtrado IP que se aplica a las interfaces de red de las instancias. Empieza con reglas que permiten todo el tráfico saliente; añada después las de entrada.";
    AddRuleIntro => "A rule allows traffic in one direction, for a protocol and ports, from (or to) a remote CIDR or the instances of a remote group. Traffic no rule allows is dropped.", "Una regla permite tráfico en un sentido, para un protocolo y unos puertos, desde (o hacia) un CIDR remoto o las instancias de un grupo remoto. El tráfico que ninguna regla permite se descarta.";
    RetypeIntro => "Changes the volume's type after its creation. When the new type lives on another back end, the volume has to be migrated.", "Cambia el tipo del volumen tras su creación. Si el tipo nuevo está en otro back end, hay que migrar el volumen.";
    CreateTransferIntro => "Ownership of a volume can move to another project: the transfer gives you an ID and a key, and the receiving project accepts it with both.", "La propiedad de un volumen puede pasar a otro proyecto: la transferencia le da un ID y una clave, y el proyecto receptor la acepta con ambos.";
    AcceptTransferIntro => "Take ownership of a volume another project transfers to this one, with the transfer ID and the authorization key that project got.", "Tome la propiedad de un volumen que otro proyecto transfiere a este, con el ID de la transferencia y la clave de autorización que obtuvo ese proyecto.";
    BackupIntro => "Backups are kept by the cloud's backup service (object storage, Ceph, NFS…), apart from the volume: restore one even after the volume is deleted.", "Las copias de seguridad las guarda el servicio de copias de la nube (almacenamiento de objetos, Ceph, NFS…), aparte del volumen: puede restaurarlas aunque se haya eliminado el volumen.";
    RestoreBackupIntro => "Restore the backup to an existing volume, overwriting it, or to a new one.", "Restaure la copia en un volumen existente, que se sobrescribe, o en uno nuevo.";
    InstanceSnapshotIntro => "A snapshot is an image that keeps the instance's disk as it is now; launch new instances from it in Images. For an instance booted from a volume, it points to snapshots of its volumes.", "Una instantánea es una imagen que conserva el disco de la instancia tal como está ahora; lance instancias nuevas desde ella en Imágenes. En una instancia arrancada desde un volumen, apunta a instantáneas de sus volúmenes.";
    UploadToImageIntro => "Copies the volume into the Image service as a new image, to launch instances from it or share it.", "Copia el volumen al servicio de imágenes como una imagen nueva, para lanzar instancias desde ella o compartirla.";
    IdentityGroupIntro => "Groups give roles to many users at once: after creating it, add users with Manage Members and give the group roles on projects.", "Los grupos dan roles a muchos usuarios a la vez: tras crearlo, añada usuarios con Gestionar miembros y dé al grupo roles en proyectos.";
    VolumeTypeIntro => "A volume type is a label users pick when creating a volume, e.g. SSD or Backup; its extra specs, set next, map it to a back end and its capabilities.", "Un tipo de volumen es una etiqueta que los usuarios eligen al crear un volumen, p. ej. SSD o Backup; sus especificaciones extra, que se fijan después, lo ligan a un back end y sus capacidades.";
    GroupTypeIntro => "A group type is a label users pick when creating a volume group; its specs, set next, map it to the back end's group capabilities.", "Un tipo de grupo es una etiqueta que los usuarios eligen al crear un grupo de volúmenes; sus especificaciones, que se fijan después, lo ligan a las capacidades de grupo del back end.";
    QosSpecIntro => "A QoS spec is a set of limits, such as IOPS or bandwidth, that applies to the volumes of the types it is associated with.", "Una especificación de QoS es un conjunto de límites, como IOPS o ancho de banda, que se aplica a los volúmenes de los tipos a los que se asocia.";
    QosAssociationIntro => "The QoS spec applied to every volume of this type. — None — removes the current association.", "La especificación de QoS que se aplica a todos los volúmenes de este tipo. — Ninguno — quita la asociación actual.";
    EncryptionIntro => "Every new volume of this type is encrypted. The encryption can't be changed while volumes of the type are in use.", "Todos los volúmenes nuevos de este tipo se cifran. El cifrado no se puede cambiar mientras haya volúmenes del tipo en uso.";
    ImportKeyIntro => "Key pairs are how you log in to an instance after launch: paste your public key here, and log in with the private one, e.g. ssh -i cloud.key <user>@<address>.", "Los pares de claves sirven para entrar en una instancia tras lanzarla: pegue aquí su clave pública, y entre con la privada, p. ej. ssh -i cloud.key <usuario>@<dirección>.";
    TipPrimaryProject => "The project the user works in by default: Keystone stores it as the user's `default_project_id`, and clients scope to it when none is given.\nIt grants **nothing** by itself: the user needs a role on it, picked below.", "El proyecto en el que trabaja el usuario por defecto: Keystone lo guarda como su `default_project_id`, y los clientes lo usan cuando no se indica otro.\nPor sí solo **no da acceso**: el usuario necesita un rol en él, que se elige abajo.";
    TipUserRole => "The role the user gets on the primary project, e.g. `member` to work in it, `reader` to only look, or `admin`.\nMore roles, on other projects too, are given later in **Identity › Projects** › *Manage Members*.", "El rol que recibe el usuario en el proyecto principal, p. ej. `member` para trabajar en él, `reader` para solo consultar, o `admin`.\nMás roles, también en otros proyectos, se dan después en **Identidad › Proyectos** › *Gestionar miembros*.";
    TipLockPassword => "The user can't change their own password through Keystone's self-service API; an administrator still can. Useful for shared or service accounts.", "El usuario no puede cambiar su propia contraseña con la API de autoservicio de Keystone; un administrador sí puede. Útil para cuentas compartidas o de servicio.";
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
    // Project › Quotas
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
    // Volume groups; *Delete Group(s)* reuse the Identity keys.
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
    ManageVolumesHelp => "Volumes with one of the group's types, available or in use, and in no other group.", "Volúmenes con uno de los tipos del grupo, disponibles o en uso, y en ningún otro grupo.";
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
    // Paged lists
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
    InterfaceIpHelp => "Optional; empty uses the subnet's gateway address.", "Opcional; vacío usa la dirección de puerta de enlace de la subred.";
    TipInterfaceIp => "The address the router gets on this subnet.\nEmpty: the subnet's gateway address (`192.168.0.1` for `192.168.0.0/24`). Fill it in when another router already has that address on the subnet: Neutron refuses two interfaces with the same IP.\nIt must be inside the subnet's range and free.", "La dirección que el router tendrá en esta subred.\nVacío: la dirección de puerta de enlace de la subred (`192.168.0.1` para `192.168.0.0/24`). Indíquela si otro router ya tiene esa dirección en la subred: Neutron no admite dos interfaces con la misma IP.\nDebe estar dentro del rango de la subred y libre.";
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
    ConsoleSize => "Console size", "Tamaño de la consola";
    ConsoleFit => "Fit", "Ajustar";
    ConsoleFitHelp => "The whole screen, scaled to the frame", "Toda la pantalla, ajustada al marco";
    ConsoleActualSize => "Actual size: sharper text, scroll to see the rest", "Tamaño real: texto más nítido, desplázate para ver el resto";
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
    HostRoutesHelp => "Extra routes for the instances, one per line: destination CIDR,next hop.", "Rutas adicionales para las instancias, una por línea: CIDR de destino,siguiente salto.";
    TipHostRoutes => "Routes that DHCP gives the subnet's instances, besides the default route through the gateway. One per line, as `destination,next hop`:\n- `192.168.200.0/24,10.56.1.254`: reach 192.168.200.0/24 through 10.56.1.254.\nThe next hop must be an address on this subnet, e.g. another router's port. Running instances pick them up when they renew their DHCP lease.", "Rutas que DHCP da a las instancias de la subred, además de la ruta por defecto a través de la puerta de enlace. Una por línea, como `destino,siguiente salto`:\n- `192.168.200.0/24,10.56.1.254`: llegar a 192.168.200.0/24 a través de 10.56.1.254.\nEl siguiente salto debe ser una dirección de esta subred, p. ej. el puerto de otro router. Las instancias en marcha las reciben al renovar su concesión DHCP.";
    AzHints => "Availability Zone Hints", "Sugerencias de zona de disponibilidad";
    AzHintsHelp => "Zones where Neutron should schedule the network's DHCP and routers; none lets it choose.", "Zonas donde Neutron debería programar el DHCP y los routers de la red; ninguna le deja elegir.";
    CreateFlavor => "Create Flavor", "Crear sabor";
    DeleteFlavor => "Delete Flavor", "Eliminar sabor";
    DeleteFlavors => "Delete Flavors", "Eliminar sabores";
    FlavorInfo => "Flavor Information", "Información del sabor";
    FlavorInfoHelp => "Flavors define the sizes for RAM, disk, number of cores, and other resources an instance gets.", "Los sabores definen el tamaño de RAM, disco, número de núcleos y otros recursos que recibe una instancia.";
    FlavorIdHelp => "Empty or `auto`: Nova generates a UUID.", "Vacío o `auto`: Nova genera un UUID.";
    TipFlavorId => "The flavor's ID in Nova; users see the name, scripts often use the ID.\n- Empty or `auto`: Nova generates a UUID.\n- Your own: letters, digits, `_`, `.`, `-` and spaces; it is **not** case-sensitive, so `M1` and `m1` clash.\nThe ID can't change later: *Edit Flavor* recreates the flavor with the same ID.", "El ID del sabor en Nova; los usuarios ven el nombre, los scripts suelen usar el ID.\n- Vacío o `auto`: Nova genera un UUID.\n- Uno propio: letras, dígitos, `_`, `.`, `-` y espacios; **no** distingue mayúsculas, así que `M1` y `m1` chocan.\nEl ID no puede cambiar después: *Editar sabor* vuelve a crear el sabor con el mismo ID.";
    TipFlavorRootDisk => "The instance's root disk on the hypervisor, in GB, where the image is copied; it is *ephemeral*, lost with the instance.\n`0` is special: the disk takes the image's own size. Instances booted from a volume don't use it.", "El disco raíz de la instancia en el hipervisor, en GB, donde se copia la imagen; es *efímero*, se pierde con la instancia.\n`0` es especial: el disco toma el tamaño de la propia imagen. Las instancias arrancadas desde un volumen no lo usan.";
    TipFlavorEphemeral => "An extra local disk, in GB, besides the root disk: storage linked to the instance's life, **lost** when it is deleted, and not included in snapshots. `0`: none.", "Un disco local adicional, en GB, además del raíz: almacenamiento ligado a la vida de la instancia, que se **pierde** al eliminarla y no entra en las instantáneas. `0`: ninguno.";
    TipFlavorSwap => "A swap disk for the instance, in **MB**, on the hypervisor's local storage. `0`: none.", "Un disco de intercambio (swap) para la instancia, en **MB**, en el almacenamiento local del hipervisor. `0`: ninguno.";
    TipRxtxFactor => "A network bandwidth weight relative to the network's base, e.g. `2.0` for double. Only some network back ends use it; `1.0` keeps the default.", "Un peso de ancho de banda de red respecto al de base de la red, p. ej. `2.0` para el doble. Solo lo usan algunos back ends de red; `1.0` mantiene el valor por defecto.";
    TipFlavorProjects => "None: the flavor is **public**, every project can use it.\nPicking projects makes it **private** to them: only they see and launch with it.", "Ninguno: el sabor es **público**, cualquier proyecto puede usarlo.\nAl elegir proyectos pasa a ser **privado** para ellos: solo ellos lo ven y lanzan con él.";
    TipProviderType => "How Neutron builds the network on the physical one:\n- **local**: only within one host; for tests.\n- **flat**: untagged, on a whole physical network.\n- **vlan**: a VLAN on a physical network.\n- **vxlan**, **geneve**, **gre**: tunnels over the hosts' network, with no physical network to name.\nThe types offered are the ones the cloud enables.", "Cómo construye Neutron la red sobre la física:\n- **local**: solo dentro de un host; para pruebas.\n- **flat**: sin etiquetar, sobre toda una red física.\n- **vlan**: una VLAN sobre una red física.\n- **vxlan**, **geneve**, **gre**: túneles sobre la red de los hosts, sin red física que indicar.\nSe ofrecen los tipos que la nube habilita.";
    TipPhysicalNetwork => "The name the cloud's Neutron configuration gives the physical network, e.g. `physnet1` or `datacentre` (its bridge mappings); ask the cloud's operators if unsure.", "El nombre que la configuración de Neutron de la nube da a la red física, p. ej. `physnet1` o `datacentre` (sus bridge mappings); en caso de duda, consulte a los operadores de la nube.";
    TipNetworkShared => "Every project can attach instances and ports to the network, not only its owner. Subnets and routers stay the owner's.", "Todos los proyectos pueden conectar instancias y puertos a la red, no solo su propietario. Las subredes y los routers siguen siendo del propietario.";
    TipNetworkExternal => "The network reaches outside the cloud through routing Neutron doesn't manage: routers use it as their **gateway**, and **floating IPs** are allocated from it.", "La red sale de la nube con un encaminamiento que Neutron no gestiona: los routers la usan como **puerta de enlace**, y de ella se asignan las **IP flotantes**.";
    TipMtu => "The largest packet, in bytes, the network carries. At least `68` for IPv4 subnets and `1280` for IPv6.\nEmpty: Neutron derives it from the physical network, minus what the tunnel takes (e.g. `1450` for VXLAN on a 1500 network).", "El paquete más grande, en bytes, que lleva la red. Al menos `68` para subredes IPv4 y `1280` para IPv6.\nVacío: Neutron lo deduce de la red física, menos lo que ocupa el túnel (p. ej. `1450` para VXLAN sobre una red de 1500).";
    TipSubnetCidr => "The subnet's address range in CIDR notation: the network address and the prefix length, e.g. `192.168.0.0/24` (254 addresses) or `2001:db8::/64` for IPv6.\nIt can't change once the subnet exists.", "El rango de direcciones de la subred en notación CIDR: la dirección de red y la longitud del prefijo, p. ej. `192.168.0.0/24` (254 direcciones) o `2001:db8::/64` para IPv6.\nNo puede cambiar una vez creada la subred.";
    TipGatewayIp => "The address of the subnet's router, given to instances as their default route, e.g. `192.168.0.254`.\nEmpty: the first address of the range (`192.168.0.1` for `192.168.0.0/24`). With *Disable Gateway*, instances get no default route; use it for networks that never leave the cloud.", "La dirección del router de la subred, que las instancias reciben como ruta por defecto, p. ej. `192.168.0.254`.\nVacío: la primera dirección del rango (`192.168.0.1` para `192.168.0.0/24`). Con *Desactivar puerta de enlace*, las instancias no reciben ruta por defecto; úselo para redes que nunca salen de la nube.";
    TipEnableDhcp => "Neutron's DHCP server gives the instances their address, gateway, DNS servers and host routes. Off: configure them inside the instance, or use a *Configuration Drive*.", "El servidor DHCP de Neutron da a las instancias su dirección, puerta de enlace, servidores DNS y rutas. Desactivado: configúrelos dentro de la instancia, o use una *Unidad de configuración*.";
    TipIpv6Mode => "How instances get their IPv6 address and the rest of their settings:\n- **SLAAC**: the instance builds its address from the router's announcements (an OpenStack router, or one outside the cloud).\n- **DHCPv6 stateful**: OpenStack's DHCP gives the address and the settings.\n- **DHCPv6 stateless**: the address by SLAAC, the settings (e.g. DNS) by DHCP.\n- **No options**: set nothing; for addresses configured another way.", "Cómo obtienen las instancias su dirección IPv6 y el resto de su configuración:\n- **SLAAC**: la instancia forma su dirección con los anuncios del router (uno de OpenStack, o uno fuera de la nube).\n- **DHCPv6 con estado**: el DHCP de OpenStack da la dirección y la configuración.\n- **DHCPv6 sin estado**: la dirección por SLAAC, la configuración (p. ej. DNS) por DHCP.\n- **Sin opciones**: no fija nada; para direcciones configuradas de otro modo.";
    TipAllocationPools => "The ranges Neutron takes addresses from for ports and instances, one per line as `start,end`, e.g. `192.168.0.100,192.168.0.200`.\nEmpty: the whole subnet but the gateway. Addresses outside the pools can still be given by hand as a fixed IP.", "Los rangos de los que Neutron toma direcciones para puertos e instancias, uno por línea como `inicio,fin`, p. ej. `192.168.0.100,192.168.0.200`.\nVacío: toda la subred menos la puerta de enlace. Las direcciones fuera de los rangos se pueden seguir dando a mano como IP fija.";
    TipDnsServers => "The DNS servers DHCP gives the subnet's instances, one address per line, e.g. `8.8.8.8`.\nEmpty: the instances use whatever the cloud's DHCP agent offers by default.", "Los servidores DNS que DHCP da a las instancias de la subred, una dirección por línea, p. ej. `8.8.8.8`.\nVacío: las instancias usan lo que ofrezca por defecto el agente DHCP de la nube.";
    TipPortSecurity => "Neutron's anti-spoofing: the port only sends traffic from its own MAC and IP addresses, and its security groups filter it.\nTurning it off also **removes the port's security groups**: no filtering is applied to its traffic. Some appliances (routers, firewalls, VPN gateways) need it off.", "El antisuplantación de Neutron: el puerto solo envía tráfico desde sus propias direcciones MAC e IP, y sus grupos de seguridad lo filtran.\nDesactivarlo también **quita los grupos de seguridad del puerto**: su tráfico no se filtra. Algunos equipos virtuales (routers, cortafuegos, pasarelas VPN) lo necesitan desactivado.";
    TipVnicType => "How the instance's interface is plugged into the network:\n- **Normal**: a virtual switch port, the usual case.\n- **Direct**, **Direct Physical**, **MacVTap**: SR-IOV, a function of the host's network card; faster, but needs supporting hosts and networks.\n- **Bare Metal**, **Virtio Forwarder**, **vDPA**, **Smart NIC**, **Remote Managed**: specialised hardware the cloud must offer.", "Cómo se conecta la interfaz de la instancia a la red:\n- **Normal**: un puerto de un switch virtual, lo habitual.\n- **Directo**, **Físico directo**, **MacVTap**: SR-IOV, una función de la tarjeta de red del host; más rápido, pero necesita hosts y redes que lo admitan.\n- **Bare metal**, **Virtio Forwarder**, **vDPA**, **Smart NIC**, **Gestionado remotamente**: hardware especializado que la nube debe ofrecer.";
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
    AggregateStepHelp => "A host aggregate groups hosts within an availability zone, e.g. those with SSDs; flavors pick them through matching metadata. A host can be in several aggregates.", "Un agregado de hosts agrupa hosts dentro de una zona de disponibilidad, p. ej. los que tienen SSD; los sabores los eligen con metadatos coincidentes. Un host puede estar en varios agregados.";
    VolumeGroupIntro => "A volume group lets you snapshot several volumes at the same point in time, so their data stays consistent.", "Un grupo de volúmenes permite crear instantáneas de varios volúmenes en el mismo instante, para que sus datos sean coherentes.";
    TipGroupVolumeTypes => "The volume types whose volumes the group can hold. Several types can share a group only if they are on the **same back end**.\nOnly volumes of these types can be added later with *Manage Volumes*.", "Los tipos de volumen cuyos volúmenes puede contener el grupo. Varios tipos solo pueden compartir grupo si están en el **mismo back end**.\nDespués solo se pueden añadir con *Gestionar volúmenes* volúmenes de estos tipos.";
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
    NetworkType => "Network Type", "Tipo de red";
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
    SourceHelp => "Booting from an image or an instance snapshot gives ephemeral storage, lost when the instance is deleted; a volume keeps the data.", "Arrancar desde una imagen o una instantánea de instancia da almacenamiento efímero, que se pierde al eliminar la instancia; un volumen conserva los datos.";
    FlavorHelp => "The flavor sets the vCPUs, memory and disk of the instance. Flavors too small for the image are marked.", "El sabor fija las vCPU, la memoria y el disco de la instancia. Se marcan los sabores demasiado pequeños para la imagen.";
    NetworksHelp => "Networks the instance connects to, in this order. You can use ports instead, or both.", "Redes a las que se conecta la instancia, en este orden. Puede usar puertos en su lugar, o ambos.";
    PortsHelp => "Existing ports to attach, after the networks.", "Puertos existentes que se conectan, después de las redes.";
    SecurityGroupsHelp => "Security groups filter the instance's traffic: without a rule that allows it, you reach the instance only from its console.", "Los grupos de seguridad filtran el tráfico de la instancia: sin una regla que lo permita, solo se llega a ella desde su consola.";
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
    ImageFileHelp => "After Create Image the browser sends the file in the background: you can keep using Moonlight, but don't close or reload this tab until it finishes.", "Tras Crear imagen, el navegador envía el archivo en segundo plano: puedes seguir usando Moonlight, pero no cierres ni recargues esta pestaña hasta que termine.";
    ChooseFile => "Choose a file…", "Elegir un archivo…";
    ImageFileTooBig => "The file is larger than the allowed", "El archivo supera el máximo permitido de";
    UploadingPct => "Uploading", "Subiendo";
    Uploaded => "Uploaded", "Subida";
    UploadKeepTabOpen => "Keep this tab open: closing or reloading it stops the upload.", "Mantén esta pestaña abierta: si la cierras o la recargas, la subida se detiene.";
    DeletingImage => "Deleting the image…", "Eliminando la imagen…";
    UploadFailedTitle => "The upload did not finish", "La subida no terminó";
    UploadFailedText => "The image was created but has no data: it stays queued. You can delete it or upload its data some other way.", "La imagen se creó pero no tiene datos: queda en cola. Puedes eliminarla o subir sus datos de otra forma.";
    CancelUpload => "Cancel upload", "Cancelar subida";
    ImageSourceNone => "None (create the record only)", "Ninguno (solo crear el registro)";
    ImageSourceNoneHelp => "With None, the image stays queued until its data is uploaded some other way.", "Con Ninguno, la imagen queda en cola hasta que se suban sus datos de otra forma.";
    ImageUrlHelp => "An http:// or https:// address the image service can reach, to a disk image.", "Una dirección http:// o https:// que el servicio de imágenes alcance, a una imagen de disco.";
    TipImageUrl => "The image service downloads the file itself (Glance's *web-download* import), so the address must be reachable **from the cloud**, not only from your browser:\n- `http://` or `https://`, with no login;\n- a disk image in the *Format* chosen below, e.g. a `.qcow2` file for QCOW2.\nThe cloud may refuse some hosts or ports, such as private addresses. The image is *importing* until the download ends.", "El servicio de imágenes descarga el fichero por sí mismo (la importación *web-download* de Glance), así que la dirección debe ser accesible **desde la nube**, no solo desde su navegador:\n- `http://` o `https://`, sin inicio de sesión;\n- una imagen de disco en el *Formato* elegido abajo, p. ej. un fichero `.qcow2` para QCOW2.\nLa nube puede rechazar algunos hosts o puertos, como las direcciones privadas. La imagen está *importando* hasta que termina la descarga.";
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
    TipDiskFormat => "The format of the image's file, which the hypervisor must support:\n- `qcow2`: compressed, grows as it is written; the usual choice for KVM.\n- `raw`: an exact disk copy; the fastest to boot from a volume on Ceph.\n- `iso`: an installation disc.\n- `vmdk`, `vhd`, `vhdx`, `vdi`: VMware, Hyper-V and VirtualBox disks.\n- `ami`, `aki`, `ari`: Amazon-style machine, kernel and ramdisk images.\nIt must match the file: Glance may refuse or misread a file whose format differs.", "El formato del fichero de la imagen, que el hipervisor debe admitir:\n- `qcow2`: comprimido, crece a medida que se escribe; lo habitual con KVM.\n- `raw`: una copia exacta del disco; la más rápida para arrancar desde un volumen en Ceph.\n- `iso`: un disco de instalación.\n- `vmdk`, `vhd`, `vhdx`, `vdi`: discos de VMware, Hyper-V y VirtualBox.\n- `ami`, `aki`, `ari`: imágenes de máquina, kernel y ramdisk al estilo de Amazon.\nDebe coincidir con el fichero: Glance puede rechazar o leer mal un fichero de otro formato.";
    TipKernelId => "The ID of the Glance image (format `aki`) used as the kernel when booting this AMI-style image. Empty: the image boots its own kernel.", "El ID de la imagen de Glance (formato `aki`) que se usa como kernel al arrancar esta imagen al estilo AMI. Vacío: la imagen arranca su propio kernel.";
    TipRamdiskId => "The ID of the Glance image (format `ari`) used as the ramdisk when booting this AMI-style image, together with the kernel.", "El ID de la imagen de Glance (formato `ari`) que se usa como ramdisk al arrancar esta imagen al estilo AMI, junto con el kernel.";
    TipMinDisk => "The root disk, in GB, the image needs to boot. Nova refuses a flavor with a smaller root disk, and *Launch Instance* sizes a new boot volume to at least this.\n`0`: no minimum (the image's file size still applies to volumes).", "El disco raíz, en GB, que la imagen necesita para arrancar. Nova rechaza un sabor con un disco raíz menor, y *Lanzar instancia* da al volumen de arranque nuevo al menos este tamaño.\n`0`: sin mínimo (a los volúmenes se les sigue aplicando el tamaño del fichero).";
    TipMinRam => "The memory, in MB, the image needs to boot. Nova refuses a flavor with less RAM, and *Launch Instance* marks those flavors. `0`: no minimum.", "La memoria, en MB, que la imagen necesita para arrancar. Nova rechaza un sabor con menos RAM, y *Lanzar instancia* marca esos sabores. `0`: sin mínimo.";
    TipImageVisibility => "Who can see and boot the image:\n- **private**: only this project.\n- **shared**: this project and the projects it is shared with (*Manage Members*); they accept it first.\n- **community**: every project can boot it, but it isn't listed to them by default.\n- **public**: every project, listed for all; only administrators can set it.", "Quién puede ver y arrancar la imagen:\n- **private**: solo este proyecto.\n- **shared**: este proyecto y los proyectos con los que se comparte (*Gestionar miembros*); antes deben aceptarla.\n- **community**: cualquier proyecto puede arrancarla, pero no se les muestra por defecto.\n- **public**: todos los proyectos, y se muestra a todos; solo los administradores pueden fijarla.";
    TipImageProtected => "A protected image can't be deleted, by anyone, until this is turned off again. Use it for images other instances or projects rely on.", "Una imagen protegida no se puede eliminar, nadie puede, hasta que se vuelva a desactivar. Úselo para imágenes de las que dependen otras instancias o proyectos.";
    TipImageMetadata => "Key-value properties stored with the image. Some only describe it, for sorting and searching; others change how instances boot from it, e.g. `hw_disk_bus`, `hw_qemu_guest_agent`, `os_distro`, `hw_firmware_type`.\n*Add from catalog* offers the properties the cloud's metadata definitions describe, with their allowed values. Keys and values are at most 255 characters.", "Propiedades clave-valor guardadas con la imagen. Algunas solo la describen, para ordenar y buscar; otras cambian cómo arrancan las instancias desde ella, p. ej. `hw_disk_bus`, `hw_qemu_guest_agent`, `os_distro`, `hw_firmware_type`.\n*Añadir del catálogo* ofrece las propiedades que describen las definiciones de metadatos de la nube, con sus valores permitidos. Claves y valores tienen como mucho 255 caracteres.";
    TipVolumeSource => "What the new volume starts with:\n- **No source, empty volume**: a blank disk, to attach and format.\n- **Image**: a copy of an image, *bootable*: launch instances from it.\n- **Snapshots**: a copy of a volume snapshot.\n- **Volume**: a clone of an available volume.\nThe size must be at least the source's.", "Con qué empieza el volumen nuevo:\n- **Sin origen, volumen vacío**: un disco en blanco, para adjuntarlo y formatearlo.\n- **Imagen**: una copia de una imagen, *arrancable*: lance instancias desde él.\n- **Instantáneas**: una copia de una instantánea de volumen.\n- **Volumen**: un clon de un volumen disponible.\nEl tamaño debe ser al menos el del origen.";
    TipVolumeType => "The type picks the storage back end and its features (speed, replication, encryption, QoS limits): with several back ends, Cinder's scheduler sends the volume where the type says.\nEmpty: the cloud's default type. A volume's type can be changed later with *Change Volume Type*.", "El tipo elige el back end de almacenamiento y sus características (velocidad, replicación, cifrado, límites de QoS): con varios back ends, el planificador de Cinder envía el volumen donde indique el tipo.\nVacío: el tipo por defecto de la nube. El tipo de un volumen se puede cambiar después con *Cambiar tipo de volumen*.";
    TipVolumeSize => "In GB (gibibytes). It must be at least the source's size: the image's minimum disk and file size, or the snapshot's or volume's size.\nA volume can be extended later, never shrunk; it counts against the project's gigabytes quota.", "En GB (gibibytes). Debe ser al menos el tamaño del origen: el disco mínimo y el tamaño del fichero de la imagen, o el de la instantánea o el volumen.\nUn volumen se puede ampliar después, nunca reducir; cuenta en la cuota de gigabytes del proyecto.";
    TipVolumeAz => "The storage zone where Cinder creates the volume; *Any Availability Zone* lets it choose. To attach the volume to an instance, or boot one from it, the zones usually have to match the instance's.", "La zona de almacenamiento donde Cinder crea el volumen; *Cualquier zona de disponibilidad* deja que elija. Para adjuntar el volumen a una instancia, o arrancar una desde él, las zonas suelen tener que coincidir con la de la instancia.";
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
    GatewayAuto => "Leave empty to use the first address of the network.", "Déjelo vacío para usar la primera dirección de la red.";
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
    DeviceId => "Device ID", "ID del dispositivo";
    MacAddressHelp => "Blank: Neutron generates one.", "En blanco: Neutron genera una.";
    PortSubnetHelp => "Blank: any subnet of the network.", "En blanco: cualquier subred de la red.";
    TipPortDevice => "What uses the port: the device's ID (an instance, a router…) and its kind, such as `compute:nova`.\nUsually left blank: attaching the port to an instance fills both.", "Lo que usa el puerto: el ID del dispositivo (una instancia, un router…) y su tipo, como `compute:nova`.\nNormalmente se deja en blanco: al conectar el puerto a una instancia se rellenan los dos.";
    CreatePortInfoHelp => "Blank fields take Neutron's defaults.", "Los campos en blanco toman los valores por defecto de Neutron.";
    CreatePortGroupsHelp => "The security groups applied to the port; with none picked, the project's default group. Only while Port Security is on.", "Los grupos de seguridad aplicados al puerto; si no elige ninguno, el grupo por defecto del proyecto. Solo con la seguridad de puertos activada.";
    AttachedDevice => "Attached Device", "Dispositivo adjunto";
    VnicType => "VNIC Type", "Tipo de VNIC";
    StepInfo => "Info", "Información";
    EditPortInfoHelp => "You may update the editable properties of your port here.", "Aquí puede actualizar las propiedades editables del puerto.";
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

    // Orchestration (Heat)
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
    // Template Generator
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

    // Alarming (Aodh)
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
    EventQueryHelp => "Optional trait filters, one per line: field op value (traits.instance_id = <ID>).", "Filtros de traits opcionales, uno por línea: campo op valor (traits.instance_id = <ID>).";
    AlarmActionsHelp => "URLs called on entering the state, one per line (webhooks, Heat signals, log://).", "URL a las que se llama al entrar en el estado, una por línea (webhooks, señales de Heat, log://).";
    AlarmNameHelp => "Shown in lists and in the notifications the alarm sends.", "Se muestra en las listas y en las notificaciones que envía la alarma.";
    AlarmDescriptionHelp => "Optional: what the alarm is for, or what to do when it fires.", "Opcional: para qué sirve la alarma o qué hacer cuando salta.";
    AlarmTypeHelp => "Where the data comes from: Prometheus, Gnocchi or events. Fixed once the alarm exists.", "De dónde vienen los datos: Prometheus, Gnocchi o eventos. No se puede cambiar una vez creada.";
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
    RepeatActionsHelp => "Call the actions on every evaluation in the state, not only on entering it.", "Llamar a las acciones en cada evaluación en el estado, no solo al entrar en él.";

    // Key Manager (Barbican)
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
    AclUsersHelp => "Keystone user IDs, separated by commas or new lines; users of other projects too.", "ID de usuario de Keystone, separados por comas o saltos de línea; también de otros proyectos.";
    TipAclUsers => "The users who may read it besides its owner, by **ID**, not by name:\n- users of **other projects** count too, e.g. a service user such as Octavia's, which must read the certificates of HTTPS listeners;\n- an admin finds the IDs in **Identity › Users**;\n- anyone can run `openstack user show <name> -f value -c id`.\nWith the project's access off, only the owner and these users can read it.", "Los usuarios que pueden leerlo además de su propietario, por **ID**, no por nombre:\n- también cuentan usuarios de **otros proyectos**, p. ej. un usuario de servicio como el de Octavia, que debe leer los certificados de los listeners HTTPS;\n- un administrador encuentra los ID en **Identidad › Usuarios**;\n- cualquiera puede usar `openstack user show <nombre> -f value -c id`.\nSin el acceso del proyecto, solo pueden leerlo el propietario y estos usuarios.";
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
    SecretTypeHelp => "What the secret holds. Services check it: Octavia wants a Certificate and a Private Key.", "Qué contiene el secreto. Los servicios lo comprueban: Octavia pide un Certificado y una Clave privada.";
    TipSecretType => "Barbican's secret type, which other services check before they use the secret:\n- **Opaque**: anything, e.g. a token or a file; the default.\n- **Passphrase**: a password or passphrase.\n- **Symmetric**: a key for a cipher such as AES, usually stored as *Binary (base64)*.\n- **Certificate**: an X.509 certificate, in PEM.\n- **Private Key**, **Public Key**: one half of a key pair, in PEM.\nOctavia's HTTPS listeners need a **Certificate** and its **Private Key**, grouped in a certificate container.", "El tipo de secreto de Barbican, que los demás servicios comprueban antes de usarlo:\n- **Opaco**: cualquier cosa, p. ej. un token o un fichero; el valor por defecto.\n- **Frase de paso**: una contraseña o frase de paso.\n- **Simétrica**: una clave para un cifrado como AES, que se suele guardar como *Binario (base64)*.\n- **Certificado**: un certificado X.509, en PEM.\n- **Clave privada**, **Clave pública**: una mitad de un par de claves, en PEM.\nLos listeners HTTPS de Octavia necesitan un **Certificado** y su **Clave privada**, agrupados en un contenedor de certificado.";
    PayloadFormatHelp => "Text for passwords and PEM files; Binary for raw bytes, pasted base64-encoded.", "Texto para contraseñas y ficheros PEM; Binario para bytes en bruto, pegados en base64.";
    TipPayloadFormat => "How the payload is sent to Barbican:\n- **Text** (`text/plain`): passwords, tokens and PEM files (certificates, keys), pasted as they are.\n- **Binary (base64)** (`application/octet-stream`): raw bytes such as a symmetric key, pasted **base64-encoded**; e.g. a new AES-256 key: `openssl rand -base64 32`.\nBarbican gives the payload back in the same format.", "Cómo se envía el contenido a Barbican:\n- **Texto** (`text/plain`): contraseñas, tokens y ficheros PEM (certificados, claves), pegados tal cual.\n- **Binario (base64)** (`application/octet-stream`): bytes en bruto como una clave simétrica, pegados **en base64**; p. ej. una clave AES-256 nueva: `openssl rand -base64 32`.\nBarbican devuelve el contenido en el mismo formato.";
    AlgorithmHelp => "Metadata stored with the secret, e.g. aes or rsa; Barbican doesn't check it.", "Metadato que se guarda con el secreto, p. ej. aes o rsa; Barbican no lo comprueba.";
    TipAlgorithm => "Informational: Barbican stores it with the secret but doesn't check it against the payload. Usual values:\n- `aes`: a symmetric key.\n- `rsa`, `dsa`, `ec`: one half of a key pair.\nOther services may read it, e.g. to pick a cipher; leave it empty when it doesn't apply.", "Informativo: Barbican lo guarda con el secreto pero no lo contrasta con el contenido. Valores habituales:\n- `aes`: una clave simétrica.\n- `rsa`, `dsa`, `ec`: una mitad de un par de claves.\nOtros servicios pueden leerlo, p. ej. para elegir un cifrado; déjelo vacío si no aplica.";
    BitLengthHelp => "The key's size in bits, e.g. 256 for AES-256 or 2048 for RSA.", "El tamaño de la clave en bits, p. ej. 256 para AES-256 o 2048 para RSA.";
    TipBitLength => "Informational, like the algorithm: Barbican doesn't check it against the payload. Usual sizes:\n- AES: `128`, `192` or `256`.\n- RSA: `2048`, `3072` or `4096`.", "Informativo, como el algoritmo: Barbican no lo contrasta con el contenido. Tamaños habituales:\n- AES: `128`, `192` o `256`.\n- RSA: `2048`, `3072` o `4096`.";
    CipherModeHelp => "The block cipher mode the key is for, e.g. cbc, gcm or ctr.", "El modo de cifrado por bloques al que se destina la clave, p. ej. cbc, gcm o ctr.";
    TipCipherMode => "Informational, like the algorithm. Usual modes for AES:\n- `cbc`: cipher block chaining, the classic mode.\n- `gcm`: Galois/counter mode, which also authenticates the data.\n- `ctr`: counter mode.\nLeave it empty for keys that aren't for a block cipher, such as RSA keys or passphrases.", "Informativo, como el algoritmo. Modos habituales para AES:\n- `cbc`: encadenamiento de bloques, el modo clásico.\n- `gcm`: modo Galois/contador, que además autentica los datos.\n- `ctr`: modo contador.\nDéjelo vacío para claves que no son de un cifrado por bloques, como las RSA o las frases de paso.";
    SecretExpirationHelp => "In UTC; after it Barbican refuses to return the secret. Empty = never expires.", "En UTC; pasada esa fecha, Barbican no devuelve el secreto. Vacío = no caduca.";
    PayloadWarning => "The secret is shown in this browser. Hide it when done.", "El secreto se muestra en este navegador. Ocúltelo al terminar.";
    PayloadBase64Note => "Binary payload, shown base64-encoded.", "Contenido binario, mostrado en base64.";
    Expired => "Expired", "Caducado";
    ExpiresSoon => "Expires within 30 days", "Caduca en menos de 30 días";
    SecretInUse => "Services using these secrets (consumers) stop working when they are deleted.", "Los servicios que usan estos secretos (consumidores) dejan de funcionar al eliminarlos.";
    SecretConsumersOf => "In use by", "En uso por";
    ContainerKeepsSecrets => "Only the container is deleted; the secrets in it stay.", "Solo se elimina el contenedor; sus secretos se conservan.";
    // Key Manager › Orders
    Orders => "Orders", "Pedidos";
    OrderKey => "Order Key", "Pedir clave";
    DeleteOrder => "Delete Order", "Eliminar pedido";
    DeleteOrders => "Delete Orders", "Eliminar pedidos";
    OrderType => "Order Type", "Tipo de pedido";
    OrderResult => "Generated", "Generado";
    OrderIntro => "Barbican generates the key itself, so its material never travels through the browser. A symmetric key is stored as a secret; an RSA key pair as a container with its public key, private key and, if you give one, the private key's passphrase. The order runs in the background: its page links what it generated once it is active.", "Barbican genera la clave por sí mismo, así que su contenido nunca pasa por el navegador. Una clave simétrica se guarda como secreto; un par de claves RSA, como un contenedor con la clave pública, la privada y, si indica una, la frase de paso de la clave privada. El pedido se procesa en segundo plano: su página enlaza lo generado cuando está activo.";
    OrderTypeHelp => "Symmetric Key: an AES key. RSA Key Pair: a public and a private key.", "Clave simétrica: una clave AES. Par de claves RSA: una clave pública y una privada.";
    TipOrderType => "Barbican generates the key itself; you don't paste any payload:\n- **Symmetric Key**: one secret, an AES key of the chosen size.\n- **RSA Key Pair**: a container with the public key, the private key and, if you give one, the passphrase, each as its own secret.", "Barbican genera la clave por sí mismo; no se pega ningún contenido:\n- **Clave simétrica**: un secreto, una clave AES del tamaño elegido.\n- **Par de claves RSA**: un contenedor con la clave pública, la clave privada y, si indica una, la frase de paso, cada una como un secreto.";
    OrderNameHelp => "Given to the order and to what it generates. Optional; unnamed ones are listed by ID.", "Se da al pedido y a lo que genere. Opcional; los que no tienen nombre se muestran por su ID.";
    OrderBitLengthHelp => "The key's size: 128, 192 or 256 bits for AES; 2048, 3072 or 4096 for RSA.", "El tamaño de la clave: 128, 192 o 256 bits para AES; 2048, 3072 o 4096 para RSA.";
    OrderModeHelp => "The block cipher mode the key is for (cbc by default). Metadata Barbican stores with it.", "El modo de cifrado por bloques de la clave (cbc por defecto). Metadato que Barbican guarda con ella.";
    OrderPassphrase => "Private Key Passphrase", "Frase de paso de la clave privada";
    OrderPassphraseHelp => "Optional. Encrypts the private key; stored in the container as a secret.", "Opcional. Cifra la clave privada; se guarda en el contenedor como un secreto.";
    OrderExpirationHelp => "When the generated secrets expire, in UTC. Empty = never expires.", "Cuándo caducan los secretos generados, en UTC. Vacío = no caducan.";
    OrderKeepsResult => "Only the order record is deleted; the secret or container it generated stays.", "Solo se elimina el registro del pedido; el secreto o contenedor que generó se conserva.";
    OrderPendingNote => "Barbican is still processing the order.", "Barbican aún está procesando el pedido.";
    ContainerSecretsHelp => "Pick a secret for each role. A certificate container (Octavia's TLS listeners) needs the certificate; an RSA one, the public and private keys.", "Elija un secreto para cada rol. Un contenedor de certificado (listeners TLS de Octavia) necesita el certificado; uno RSA, las claves pública y privada.";

    // Trunks
    Trunks => "Trunks", "Trunks";
    CreateTrunk => "Create Trunk", "Crear trunk";
    EditTrunk => "Edit Trunk", "Editar trunk";
    DeleteTrunk => "Delete Trunk", "Eliminar trunk";
    DeleteTrunks => "Delete Trunks", "Eliminar trunks";
    // Firewall (FWaaS v2): neutron-fwaas-dashboard's names
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
    CreateTrunkIntro => "A trunk carries several networks over one instance interface: the parent port's network untagged, and each subport's network as VLAN-tagged traffic.", "Un trunk lleva varias redes por una sola interfaz de la instancia: la red del puerto padre sin etiquetar, y la de cada subpuerto como tráfico etiquetado con VLAN.";
    TipTrunkParent => "Exactly one free port, fixed for the trunk's life: it can't be the parent or a subport of another trunk.\nLaunch the instance with **this port** (*Network Ports* step), not with the trunk or its subports. Inside the instance its network is the **untagged** one, available from boot.\nSome back ends, notably Open vSwitch, only allow a trunk on a port that no instance uses yet.", "Exactamente un puerto libre, fijo durante toda la vida del trunk: no puede ser el padre ni un subpuerto de otro trunk.\nLance la instancia con **este puerto** (paso *Puertos de red*), no con el trunk ni con sus subpuertos. Dentro de la instancia su red es la **sin etiquetar**, disponible desde el arranque.\nAlgunos back ends, como Open vSwitch, solo permiten un trunk en un puerto que aún no usa ninguna instancia.";
    TipTrunkAdminState => "Off: subports can't be added or removed and the trunk can't be deleted until it is turned on again. Traffic keeps flowing.", "Desactivado: no se pueden añadir ni quitar subpuertos ni eliminar el trunk hasta volver a activarlo. El tráfico sigue pasando.";
    TipSubport => "A free port whose network reaches the instance as **tagged** frames on the parent port's interface. Subports can be added and removed at any time, also while the instance runs; their networks work once the trunk is *ACTIVE*.\nMost guest systems don't bring up the VLAN interface by themselves, e.g.:\n- `sudo ip link add link eth0 name eth0.101 address <subport MAC> type vlan id 101`\n- `sudo dhclient eth0.101`\nGiving the subports the parent's MAC address when creating them saves the `address` part.", "Un puerto libre cuya red llega a la instancia como tramas **etiquetadas** en la interfaz del puerto padre. Los subpuertos se pueden añadir y quitar en cualquier momento, también con la instancia en marcha; sus redes funcionan cuando el trunk está *ACTIVE*.\nLa mayoría de los sistemas invitados no levantan solos la interfaz VLAN, p. ej.:\n- `sudo ip link add link eth0 name eth0.101 address <MAC del subpuerto> type vlan id 101`\n- `sudo dhclient eth0.101`\nDar a los subpuertos la MAC del padre al crearlos ahorra la parte `address`.";
    TipSegmentationType => "How the instance tells the subport's traffic apart:\n- **vlan**: a VLAN ID you choose, 1-4094, unique within the trunk. It is independent of how Neutron builds the network: frames are retagged as they leave or enter the instance.\n- **inherit**: for ports on provider networks, the type and ID are taken from the network's own segmentation; for switches that can't retag, e.g. with bare-metal (Ironic) instances.", "Cómo distingue la instancia el tráfico del subpuerto:\n- **vlan**: un ID de VLAN que usted elige, 1-4094, único dentro del trunk. Es independiente de cómo construye Neutron la red: las tramas se reetiquetan al salir o entrar en la instancia.\n- **inherit**: para puertos de redes de proveedor, el tipo y el ID se toman de la segmentación de la propia red; para switches que no pueden reetiquetar, p. ej. con instancias bare metal (Ironic).";
    SubportNotParent => "A sub-port cannot be the parent port.", "Un subpuerto no puede ser el puerto padre.";

    CliEquivalent => "Command Line equivalent", "Equivalente en línea de comandos";
    CliEquivalentHelp => "You may use the following OpenStack CLI command(s) to perform the same action.",
        "Puede usar los siguientes comandos de la CLI de OpenStack para realizar la misma acción.";
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
    ShowPassword => "Show password", "Mostrar contraseña";
    AdminPassword => "Admin Password", "Contraseña de administrador";
    AdminPasswordHelp => "Set by the hypervisor on first boot; leave empty to let the image decide.", "La fija el hipervisor en el primer arranque; déjela vacía para que decida la imagen.";
    TipLaunchAz => "The zone where Nova places the instance; *Any Availability Zone* lets the scheduler choose.\nTo boot from an existing **volume** or **volume snapshot**, pick the zone the volume is in, or leave *Any Availability Zone* if the cloud allows attaching volumes across zones.", "La zona donde Nova coloca la instancia; *Cualquier zona de disponibilidad* deja que elija el planificador.\nPara arrancar desde un **volumen** o una **instantánea de volumen** existente, elija la zona en la que está el volumen, o deje *Cualquier zona de disponibilidad* si la nube permite adjuntar volúmenes entre zonas.";
    TipLaunchCount => "How many identical instances to create in one request. With more than one, Nova adds a number to each name (`web-1`, `web-2`…).\nBooting from an existing **volume** allows only **one**: a volume attaches to one instance.", "Cuántas instancias iguales se crean en una petición. Con más de una, Nova añade un número a cada nombre (`web-1`, `web-2`…).\nArrancar desde un **volumen** existente solo permite **una**: un volumen se adjunta a una sola instancia.";
    TipBootSource => "Where the instance's disk comes from:\n- **Image**, **Instance Snapshot**: copied to the hypervisor's local disk, *ephemeral*: lost when the instance is deleted. With *Create New Volume* it goes to a new volume instead.\n- **Volume**: an existing bootable volume, *persistent*; no copy is made, and only one instance can boot from it.\n- **Volume Snapshot**: a new volume is created from the snapshot, *persistent*.", "De dónde sale el disco de la instancia:\n- **Imagen**, **Instantánea de instancia**: se copia al disco local del hipervisor, *efímero*: se pierde al eliminar la instancia. Con *Crear volumen nuevo* va a un volumen nuevo en su lugar.\n- **Volumen**: un volumen arrancable existente, *persistente*; no se copia, y solo una instancia puede arrancar desde él.\n- **Instantánea de volumen**: se crea un volumen nuevo a partir de la instantánea, *persistente*.";
    TipCreateNewVolume => "**Yes**: Cinder creates a volume from the image and the instance boots from it, so its data **persists** and the disk can be bigger than the flavor's.\n**No**: the image is copied to the hypervisor's local disk, sized by the flavor, and lost when the instance is deleted.", "**Sí**: Cinder crea un volumen a partir de la imagen y la instancia arranca desde él, así que sus datos **persisten** y el disco puede ser mayor que el del sabor.\n**No**: la imagen se copia al disco local del hipervisor, con el tamaño del sabor, y se pierde al eliminar la instancia.";
    TipLaunchVolumeSize => "The size of the new boot volume, in GB. It must fit the image: Moonlight raises it to the image's minimum disk and size, and to the flavor's root disk, never lowering what you typed.\nThe volume counts against the project's volume quota, not the flavor.", "El tamaño del volumen de arranque nuevo, en GB. Debe caber la imagen: Moonlight lo sube al disco mínimo y al tamaño de la imagen, y al disco raíz del sabor, sin bajar nunca lo que haya escrito.\nEl volumen cuenta en la cuota de volúmenes del proyecto, no en el sabor.";
    TipDeleteOnTerminate => "**Yes**: the boot volume is deleted with the instance.\n**No**: it stays in *Volumes* when the instance is deleted, with the instance's data; you can boot a new instance from it or delete it yourself (it keeps using quota).", "**Sí**: el volumen de arranque se elimina con la instancia.\n**No**: se queda en *Volúmenes* al eliminar la instancia, con sus datos; puede arrancar una instancia nueva desde él o eliminarlo usted (sigue ocupando cuota).";
    TipLaunchFlavor => "A flavor fixes the **vCPUs**, **RAM**, **root disk** and any ephemeral disk and swap of the instance, plus extra specs such as CPU pinning.\nA flavor too small for the image's minimum RAM or disk is marked and can't be picked. When booting from a volume, the root disk size doesn't apply.\nAdministrators create flavors, also private ones for a project: ask them if none fits.", "Un sabor fija las **vCPU**, la **RAM**, el **disco raíz** y el disco efímero y swap, si los hay, además de especificaciones extra como la fijación de CPU.\nUn sabor demasiado pequeño para la RAM o el disco mínimos de la imagen se marca y no se puede elegir. Al arrancar desde un volumen, el tamaño del disco raíz no se aplica.\nLos administradores crean los sabores, también privados para un proyecto: pídales uno si ninguno sirve.";
    TipLaunchNetworks => "One network interface per network, in this order (the first is usually the default route):\n- **Project networks**: your own, isolated from other projects.\n- **Provider** and **shared** networks: set up by administrators, mapped to a network of the data centre.\n- **External** networks: to reach outside the cloud, connect a router between your network and the external one (*Network Topology*).\nA **floating IP** is associated after launch, from the instance's *Associate Floating IP*.", "Una interfaz de red por red, en este orden (la primera suele llevar la ruta por defecto):\n- **Redes del proyecto**: las suyas, aisladas de otros proyectos.\n- Redes **de proveedor** y **compartidas**: las crean los administradores, ligadas a una red del centro de datos.\n- Redes **externas**: para salir de la nube, conecte un router entre su red y la externa (*Topología de red*).\nUna **IP flotante** se asocia tras el lanzamiento, desde *Asociar IP flotante* de la instancia.";
    TipLaunchPorts => "A port is a plug on a network that already has its **MAC address** and **IP addresses**, and its own security groups; the instance's interface takes them over.\nOnly free ports are offered: admin state up and no device attached. Use one to keep a fixed address, or for a trunk's parent port.", "Un puerto es una conexión a una red que ya tiene su **dirección MAC** y sus **direcciones IP**, y sus propios grupos de seguridad; la interfaz de la instancia los toma.\nSolo se ofrecen puertos libres: estado administrativo activo y sin dispositivo. Úselo para conservar una dirección fija, o para el puerto padre de un trunk.";
    TipLaunchSecurityGroups => "Each group is a set of rules allowing traffic **to** and **from** the instance; traffic no rule allows is dropped. The *default* group lets the group's instances talk to each other and allows all outgoing traffic, but **no incoming** traffic from elsewhere.\nAdd rules, e.g. SSH on port 22, in **Network › Security Groups** › *Manage Rules*; changes apply at once to running instances.\nThey apply to the interfaces made from networks; chosen ports keep their own groups.", "Cada grupo es un conjunto de reglas que permiten tráfico **hacia** y **desde** la instancia; el tráfico que ninguna regla permite se descarta. El grupo *default* deja que sus instancias se comuniquen entre sí y permite todo el tráfico saliente, pero **ningún tráfico entrante** de otros sitios.\nAñada reglas, p. ej. SSH en el puerto 22, en **Red › Grupos de seguridad** › *Gestionar reglas*; los cambios se aplican en el acto a las instancias en marcha.\nSe aplican a las interfaces creadas desde redes; los puertos elegidos conservan sus propios grupos.";
    TipLaunchKeyPair => "Its public key is put in the instance (by cloud-init), so you log in with the private key: `ssh -i cloud.key <user>@<address>`.\nTo make one on Linux or macOS: `ssh-keygen -t ed25519 -f cloud.key` gives `cloud.key` (private) and `cloud.key.pub` (public); import the `.pub` in **Compute › Key Pairs**, or create one there. On Windows, PuTTYgen does the same.\nThe user name depends on the image, e.g. `ubuntu`, `cloud-user`.", "Su clave pública se pone en la instancia (con cloud-init), así que se entra con la clave privada: `ssh -i cloud.key <usuario>@<dirección>`.\nPara crear uno en Linux o macOS: `ssh-keygen -t ed25519 -f cloud.key` da `cloud.key` (privada) y `cloud.key.pub` (pública); importe la `.pub` en **Compute › Pares de claves**, o cree uno allí. En Windows, PuTTYgen hace lo mismo.\nEl usuario depende de la imagen, p. ej. `ubuntu`, `cloud-user`.";
    TipCustomizationScript => "Passed to the instance as **user data**, run on first boot by cloud-init (or cloudbase-init on Windows), e.g. to install packages or add a user:\n- a shell script starting with `#!/bin/bash`;\n- or a cloud-config file starting with `#cloud-config`.\nType it, or load it from a file; it may be at most 16 KB. The image must include cloud-init for it to run.", "Se pasa a la instancia como **user data**, que cloud-init (o cloudbase-init en Windows) ejecuta en el primer arranque, p. ej. para instalar paquetes o añadir un usuario:\n- un script de shell que empiece por `#!/bin/bash`;\n- o un fichero cloud-config que empiece por `#cloud-config`.\nEscríbalo o cárguelo de un fichero; puede ocupar como mucho 16 KB. La imagen debe incluir cloud-init para que se ejecute.";
    TipDiskPartition => "- **Automatic**: the root disk is resized to the flavor's size as a single partition.\n- **Manual**: the disk keeps the image's partitions and the rest stays unallocated, to partition it yourself.", "- **Automático**: el disco raíz se amplía al tamaño del sabor como una sola partición.\n- **Manual**: el disco conserva las particiones de la imagen y el resto queda sin asignar, para particionarlo usted.";
    TipConfigDrive => "Writes the metadata, the user data and the key to a small read-only disk attached to the instance, which cloud-init reads at boot.\nTurn it on when the instance can't reach the metadata service (`169.254.169.254`), e.g. on a network without DHCP or a router.", "Escribe los metadatos, el user data y la clave en un pequeño disco de solo lectura adjunto a la instancia, que cloud-init lee al arrancar.\nActívelo si la instancia no alcanza el servicio de metadatos (`169.254.169.254`), p. ej. en una red sin DHCP o sin router.";
    TipLaunchServerGroup => "A server group places its instances by its policy:\n- **affinity**: on the same host, for low latency between them;\n- **anti-affinity**: on different hosts, so one host failure doesn't take them all;\n- **soft-** variants: the same, when the scheduler can.\nThe group is fixed at launch: an instance can't join or leave it later. Server groups belong to the project.", "Un grupo de servidores coloca sus instancias según su política:\n- **afinidad**: en el mismo host, para una latencia baja entre ellas;\n- **antiafinidad**: en hosts distintos, para que el fallo de un host no las tumbe todas;\n- variantes **soft-**: lo mismo, cuando el planificador puede.\nEl grupo se fija al lanzar: una instancia no puede entrar ni salir de él después. Los grupos de servidores son del proyecto.";
    HidePassword => "Hide password", "Ocultar contraseña";
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
    LbAdvancedHelp => "Timeouts, headers and allowed networks. Empty fields keep Octavia's defaults.", "Tiempos de espera, cabeceras y redes permitidas. Vacío: el valor por defecto de Octavia.";
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
    LbCertificateHelp => "A certificate container with a private key, or a PKCS#12 bundle, from Key Manager.", "Un contenedor de certificado con clave privada, o un paquete PKCS#12, del gestor de claves.";
    TipLbCertificate => "Octavia ends HTTPS with this certificate (*TLS termination*) and sends plain HTTP to the members. It comes from **Key Manager**:\n- a **certificate container** with the certificate, its private key and any intermediates;\n- or a **PKCS#12** bundle stored as one secret.\nOctavia must be able to read it: give its service user access with *Edit Access* on the container and on each of its secrets.", "Octavia termina HTTPS con este certificado (*terminación TLS*) y envía HTTP sin cifrar a los miembros. Viene del **gestor de claves**:\n- un **contenedor de certificado** con el certificado, su clave privada y los intermedios, si los hay;\n- o un paquete **PKCS#12** guardado como un solo secreto.\nOctavia debe poder leerlo: dé acceso a su usuario de servicio con *Editar acceso* en el contenedor y en cada uno de sus secretos.";
    MembersFieldHelp => "Instance addresses, or any address on a subnet the load balancer reaches.", "Direcciones de instancia, o cualquier dirección en una subred que alcance el balanceador.";
    TipMembers => "The servers that receive the traffic, one row each:\n- **IP Address** and **Subnet**: an instance's address, or any address on a subnet the load balancer reaches.\n- **Port**: where the member listens, e.g. `80`; it can differ from the listener's.\n- **Weight**: its share of the traffic, `0` to `256`; `0` sends it no new connections without removing it.\nA member that fails the health monitor gets no traffic until it passes again.", "Los servidores que reciben el tráfico, uno por fila:\n- **Dirección IP** y **Subred**: la dirección de una instancia, o cualquier dirección en una subred que alcance el balanceador.\n- **Puerto**: donde escucha el miembro, p. ej. `80`; puede ser distinto del del listener.\n- **Peso**: su parte del tráfico, de `0` a `256`; con `0` no recibe conexiones nuevas, sin quitarlo.\nUn miembro que no supera el monitor de salud no recibe tráfico hasta que vuelve a superarlo.";
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
    // *Add Rule* help under each field.
    DirectionHelp => "Ingress: traffic arriving at the instances. Egress: traffic leaving them.", "Entrada: tráfico que llega a las instancias. Salida: tráfico que sale de ellas.";
    EtherTypeHelp => "IPv4 or IPv6; it must match the family of the remote CIDR.", "IPv4 o IPv6; debe coincidir con la familia del CIDR remoto.";
    IpProtocolHelp => "Empty: any protocol (then ports don't apply). Otherwise TCP, UDP or ICMP.", "Vacío: cualquier protocolo (los puertos no se aplican). Si no, TCP, UDP o ICMP.";
    PortFromHelp => "A port (1-65535) or the first of a range; empty: every port. ICMP: the type.", "Un puerto (1-65535) o el primero de un rango; vacío: todos. En ICMP, el tipo.";
    TipPortFrom => "Depends on the protocol:\n- **TCP, UDP**: the port, or the first of a range ending at *To Port*; e.g. `22` for SSH, `443` for HTTPS. Empty: every port.\n- **ICMP**: the ICMP type, e.g. `8` echo request (ping), `0` echo reply, `3` destination unreachable. Empty: every type.\n- **No protocol** (any): no port.", "Depende del protocolo:\n- **TCP, UDP**: el puerto, o el primero de un rango que acaba en *Puerto final*; p. ej. `22` para SSH, `443` para HTTPS. Vacío: todos los puertos.\n- **ICMP**: el tipo ICMP, p. ej. `8` solicitud de eco (ping), `0` respuesta de eco, `3` destino inalcanzable. Vacío: todos los tipos.\n- **Sin protocolo** (cualquiera): sin puerto.";
    PortToHelp => "The last port of the range. Empty: the same as From Port. For ICMP, the ICMP code.", "El último puerto del rango. Vacío: el mismo que el puerto inicial. En ICMP, el código ICMP.";
    RemoteCidrHelp => "The addresses the rule allows: 0.0.0.0/0 is anywhere in IPv4, ::/0 anywhere in IPv6.", "Las direcciones que permite la regla: 0.0.0.0/0 es cualquiera en IPv4, ::/0 cualquiera en IPv6.";
    RemoteGroupHelp => "Allow traffic from the instances in this group; the CIDR is then ignored.", "Permite el tráfico de las instancias de este grupo; el CIDR se ignora entonces.";
    TipRemoteGroup => "The rule then matches traffic from any port that has this group, whatever its address:\n- pick the rule's own group so that its instances can reach each other;\n- pick another group, e.g. *web*, to let only those instances reach this one.\nThe addresses follow the group: instances that join or leave it are covered at once.", "La regla coincide entonces con el tráfico de cualquier puerto que tenga este grupo, sea cual sea su dirección:\n- elija el propio grupo de la regla para que sus instancias se comuniquen entre sí;\n- elija otro grupo, p. ej. *web*, para que solo esas instancias lleguen a esta.\nLas direcciones siguen al grupo: las instancias que entran o salen de él quedan cubiertas en el acto.";
    RuleDescriptionHelp => "Optional; shown in the rules table.", "Opcional; se muestra en la tabla de reglas.";
    Remote => "Remote", "Remoto";

    // Floating IPs
    AllocateIp => "Allocate IP To Project", "Asignar IP al proyecto";
    ReleaseIp => "Release Floating IP", "Liberar IP flotante";
    Associate => "Associate", "Asociar";
    Disassociate => "Disassociate", "Desasociar";
    PortToAssociate => "Port to be associated", "Puerto que se asociará";

    // Quick wins
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

    // Error page
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
    ErrTooLargeTitle => "List too large", "Lista demasiado grande";
    ErrTooLargeText => "This list has more items than this view loads at once. Use the paged lists under Admin, or ask the operator to raise [ui] max_list_items.", "Esta lista tiene más elementos de los que esta vista carga de una vez. Usa las listas paginadas de Administración o pide al operador que aumente [ui] max_list_items.";
    LoginThrottled => "Too many failed sign-ins. Wait a few minutes and try again.", "Demasiados inicios de sesión fallidos. Espera unos minutos y vuelve a intentarlo.";
    ErrInternalText => "An unexpected error occurred while loading this page.", "Se ha producido un error inesperado al cargar esta página.";
    RegionFallback => "The region you were using is not available any more. Moonlight switched to", "La región que usabas ya no está disponible. Moonlight ha cambiado a";

    // Theme
    Theme => "Theme", "Tema";
    ThemeLight => "Light", "Claro";
    ThemeDark => "Dark", "Oscuro";
    ThemeSystem => "Auto", "Automático";

    // Network Topology
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
    ShowAsList => "Show as list", "Ver como lista";
    Navigation => "Navigation", "Navegación";
    Account => "Account", "Cuenta";
    TopologyEmpty => "This project has no networks, routers or instances.", "Este proyecto no tiene redes, routers ni instancias.";
    TopologyHint => "Drag to pan, scroll to zoom. Click a node to open it.", "Arrastra para desplazar y usa la rueda para ampliar. Haz clic en un nodo para abrirlo.";
    Connections => "Connections", "Conexiones";
    Router => "Router", "Router";
    Gateway => "gateway", "puerta de enlace";
    // Instance actions parity
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
    EditSecurityGroupsHelp => "Applied to every port of the instance; for one port, use Edit Port Security Groups.", "Se aplican a todos los puertos de la instancia; para uno solo, use Editar grupos de seguridad del puerto.";
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
    // Volume actions parity
    ChangeVolumeType => "Change Volume Type", "Cambiar tipo de volumen";
    NewVolumeType => "Type", "Tipo";
    MigrateOnDemand => "Allow migration to another backend", "Permitir la migración a otro backend";
    MigrateHelp => "Needed when the new type lives on a different backend. The data is copied, which can take long.", "Necesario si el tipo nuevo está en otro backend. Se copian los datos, lo que puede tardar.";
    SameVolumeType => "Choose a type different from the current one.", "Elige un tipo distinto del actual.";
    UploadToImage => "Upload to Image", "Subir a imagen";
    ForceUpload => "Force (the volume is attached)", "Forzar (el volumen está adjunto)";
    CreateTransfer => "Create Transfer", "Crear transferencia";
    TransferName => "Transfer Name", "Nombre de la transferencia";
    TransferHelp => "The receiving project needs the transfer ID and the authorization key you get next.", "El proyecto receptor necesita el ID de la transferencia y la clave de autorización que obtendrá.";
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
    ExpirationHelp => "In UTC. Empty = never expires.", "En UTC. Vacío = no caduca.";
    TipExpiration => "The expiration is a moment in **UTC**:\n- a date with no time means **00:00** UTC;\n- a time with no date means **today**;\n- both empty: it never expires.", "La caducidad es un momento en **UTC**:\n- una fecha sin hora significa las **00:00** UTC;\n- una hora sin fecha significa **hoy**;\n- ambas vacías: no caduca.";
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
    AccessRulesHelp => "Optional: a JSON or YAML list of the only calls the credential can make. For example:", "Opcional: una lista JSON o YAML de las únicas llamadas que puede hacer la credencial. Por ejemplo:";
    TipAccessRules => "Each rule allows one kind of API call, and the credential can then make *only* the calls some rule allows:\n- `service`: the service type in the catalog, e.g. `compute`, `image`, `network`, `volumev3`.\n- `method`: the HTTP method, e.g. `GET`, `POST`, `DELETE`.\n- `path`: the API path without the endpoint's address; `*` matches one segment and `**` any number of them, e.g. `/v2.1/servers/*`.\nEmpty: no restriction, the credential can do whatever its roles allow.", "Cada regla permite un tipo de llamada a la API, y la credencial solo puede hacer las llamadas que permita *alguna* regla:\n- `service`: el tipo de servicio del catálogo, p. ej. `compute`, `image`, `network`, `volumev3`.\n- `method`: el método HTTP, p. ej. `GET`, `POST`, `DELETE`.\n- `path`: la ruta de la API sin la dirección del endpoint; `*` coincide con un segmento y `**` con cualquier número de ellos, p. ej. `/v2.1/servers/*`.\nVacío: sin restricción, la credencial puede hacer todo lo que le permitan sus roles.";
    AppCredHelp => "An application credential lets an application authenticate as you, with your roles in the current project (or some of them), without your password.", "Una credencial de aplicación permite a una aplicación autenticarse como usted, con sus roles en el proyecto actual (o parte de ellos), sin su contraseña.";
    VolumeMessages => "Messages", "Mensajes";
    MessageLevel => "Level", "Nivel";
    // Admin › System Information
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
    // Admin › Compute, Volume, Network
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
    StoragePools => "Storage Pools", "Pools de almacenamiento";
    StorageSummary => "Storage Summary", "Resumen de almacenamiento";
    CapacityUsage => "Capacity Usage", "Uso de capacidad";
    AllocatedCapacity => "Allocated", "Asignada";
    ProvisionedCapacity => "Provisioned", "Aprovisionada";
    Backend => "Backend", "Backend";
    Vendor => "Vendor", "Fabricante";
    DriverVersion => "Driver Version", "Versión del controlador";
    Capacity => "Capacity", "Capacidad";
    TotalGb => "Total (GB)", "Total (GB)";
    FreeGb => "Free (GB)", "Libre (GB)";
    AllocatedGb => "Allocated (GB)", "Asignada (GB)";
    ProvisionedGb => "Provisioned (GB)", "Aprovisionada (GB)";
    Reserved => "Reserved", "Reservada";
    OverSubscription => "Over-subscription", "Sobresuscripción";
    Provisioning => "Provisioning", "Aprovisionamiento";
    Thin => "Thin", "Fino";
    Thick => "Thick", "Grueso";
    Infinite => "Infinite", "Infinita";
    VendorCapabilities => "Vendor Capabilities", "Capacidades del fabricante";
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
    // Modules
    Modules => "Detected capabilities", "Capacidades detectadas";
    ModulesUnknown => "Not detected yet: every panel and action is shown.", "Aún sin detectar: se muestran todos los paneles y acciones.";
    ComputeApi => "Compute API (Nova)", "API de cómputo (Nova)";
    VolumeApi => "Volume API (Cinder)", "API de volúmenes (Cinder)";
    LoadBalancerApi => "Load Balancer API (Octavia)", "API de balanceadores (Octavia)";
    HiddenPanels => "Hidden panels", "Paneles ocultos";
    NetworkExtensions => "Network extensions", "Extensiones de red";
    // Bulk actions
    SelectRow => "Select", "Seleccionar";
    SelectAllRows => "Select all visible rows", "Seleccionar todas las filas visibles";
    DeleteInstances => "Delete Instances", "Eliminar instancias";
    DeleteImages => "Delete Images", "Eliminar imágenes";
    DeleteKeyPairs => "Delete Key Pairs", "Eliminar pares de claves";
    DeleteServerGroups => "Delete Server Groups", "Eliminar grupos de servidores";
    DeleteVolumes => "Delete Volumes", "Eliminar volúmenes";
    DeleteSnapshots => "Delete Volume Snapshots", "Eliminar instantáneas de volumen";
    DeleteNetworks => "Delete Networks", "Eliminar redes";
    DeleteInterfaces => "Delete Interfaces", "Eliminar interfaces";
    DeleteSubnets => "Delete Subnets", "Eliminar subredes";
    DeletePorts => "Delete Ports", "Eliminar puertos";
    DeleteListeners => "Delete Listeners", "Eliminar listeners";
    DeletePools => "Delete Pools", "Eliminar pools";
    DeleteMembers => "Delete Members", "Eliminar miembros";
    DeleteHealthMonitors => "Delete Health Monitors", "Eliminar monitores de estado";
    DeleteL7Policies => "Delete L7 Policies", "Eliminar políticas L7";
    DeleteL7Rules => "Delete L7 Rules", "Eliminar reglas L7";
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
    let range = if shown == 0 {
        "0".to_string()
    } else {
        format!("{from}–{to}")
    };
    let count = count.map(|(n, more)| if more { format!("{n}+") } else { n.to_string() });
    match (lang, count) {
        (Lang::EnUs, None) => format!("Items {range}"),
        (Lang::EnUs, Some(n)) => format!("Items {range} of {n}"),
        (Lang::EsEs, None) => format!("Elementos {range}"),
        (Lang::EsEs, Some(n)) => format!("Elementos {range} de {n}"),
    }
}

/// Tip of *Show deleted* for a user without the `admin` role: Nova shows them deleted servers
/// only among what changed in the last `days` days (`[ui] deleted_instances_days`).
pub fn show_deleted_tip(lang: Lang, days: u32) -> String {
    match lang {
        Lang::EnUs => format!(
            "Also list the instances deleted in the last {days} days, to see what an instance was. \
             Nova shows deleted instances to non-admins only among those that changed since then, so \
             the list keeps only the instances that changed in that time. Deleted rows have no actions \
             and no detail page."
        ),
        Lang::EsEs => format!(
            "Lista también las instancias eliminadas en los últimos {days} días, para ver qué era una \
             instancia. Nova solo muestra instancias eliminadas a quien no es administrador entre las \
             que cambiaron desde entonces, así que la lista se queda con las instancias que cambiaron \
             en ese tiempo. Las filas eliminadas no tienen acciones ni página de detalle."
        ),
    }
}

/// Footer count of a table with no paging ("Displaying 3 items"), singular for one.
pub fn displaying(lang: Lang, n: usize) -> String {
    match (lang, n) {
        (Lang::EnUs, 1) => "Displaying 1 item".into(),
        (Lang::EnUs, n) => format!("Displaying {n} items"),
        (Lang::EsEs, 1) => "Mostrando 1 elemento".into(),
        (Lang::EsEs, n) => format!("Mostrando {n} elementos"),
    }
}

/// *Network Topology* over `[network] topology_max_networks`.
pub fn topology_too_many(lang: Lang, n: usize, max: usize) -> String {
    match lang {
        Lang::EnUs => format!("This project sees {n} networks, more than the {max} the topology draws."),
        Lang::EsEs => format!("Este proyecto ve {n} redes, más de las {max} que dibuja la topología."),
    }
}

/// Bulk button label with the selection size: "Delete Volumes (3)".
pub fn with_count(lang: Lang, k: K, n: usize) -> String {
    if n == 0 {
        t(lang, k).to_string()
    } else {
        format!("{} ({n})", t(lang, k))
    }
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
///.
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
    const EN: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const ES: [&str; 12] = [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ];
    let i = (m.clamp(1, 12) - 1) as usize;
    if lang == Lang::EsEs { ES[i] } else { EN[i] }
}

/// Monday-first weekday initials for the date picker.
pub fn weekday_initials(lang: Lang) -> &'static [&'static str; 7] {
    if lang == Lang::EsEs {
        &["L", "M", "X", "J", "V", "S", "D"]
    } else {
        &["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
    }
}

/// Nova server group policy (`affinity`, `soft-anti-affinity`, …) in words; an unknown policy
/// is shown as Nova sent it.
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
        // A FWaaS firewall group that isn't applied: no ports, or no policy.
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
        // A Barbican order being processed.
        "pending" => ("Pending", "Pendiente"),
        _ => return title_case(status),
    };
    match lang {
        Lang::EnUs => en.into(),
        Lang::EsEs => es.into(),
    }
}

/// A Heat status (`CREATE_COMPLETE`, `UPDATE_IN_PROGRESS`…):
/// action and state translated apart, as Horizon shows them ("Create Complete").
pub fn stack_status(lang: Lang, status: &str) -> String {
    let Some((action, state)) = ["_IN_PROGRESS", "_COMPLETE", "_FAILED"]
        .iter()
        .find_map(|st| status.strip_suffix(st).map(|a| (a, &st[1..])))
    else {
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

/// Barbican's secret and container types and container roles,
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
        // Order types.
        "key" => ("Symmetric Key", "Clave simétrica"),
        "asymmetric" => ("RSA Key Pair", "Par de claves RSA"),
        _ => return title_case(term),
    };
    match lang {
        Lang::EnUs => en.into(),
        Lang::EsEs => es.into(),
    }
}

/// Fallback label for a status or term with no translation: lower case, `_` as spaces,
/// first letter capitalised (`SHUTOFF_X` → "Shutoff x").
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
    // Never `-0.00`.
    let s = crate::model::fixed(v, places);
    match lang {
        Lang::EnUs => s,
        Lang::EsEs => s.replace('.', ","),
    }
}

/// [`decimal`] with the integer part grouped by thousands: `1,234,567.89` in
/// English, `1.234.567,89` in Spanish. Spanish leaves four-digit
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

/// A background job's progress: `12 / 40`, grouped like [`grouped`].
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
        Lang::EnUs => {
            format!("Stopped after {done} of {total}: the session expired. Sign in again; the rest was not sent.")
        }
        Lang::EsEs => format!(
            "Detenido tras {done} de {total}: la sesión ha caducado. Inicie sesión de nuevo; el resto no se ha enviado."
        ),
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
        Lang::EnUs => {
            format!("Provider network: {parts}. Set when the network was created; Neutron doesn't change it.")
        }
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

/// An upload's speed and time left: `14 MB/s · about 2 min left`.
pub fn upload_rate(lang: Lang, per_s: f64, left_s: f64) -> String {
    let speed = format!("{}/s", bytes(per_s as u64));
    let left = left_s.ceil() as u64;
    let (n, unit_en, unit_es) = match left {
        0..60 => (left, "s", "s"),
        60..3600 => (left.div_ceil(60), "min", "min"),
        _ => (left / 3600, "h", "h"),
    };
    match lang {
        Lang::EnUs => format!("{speed} · about {n} {unit_en} left"),
        Lang::EsEs => format!("{speed} · quedan unos {n} {unit_es}"),
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
        assert_eq!(
            page_range(Lang::EnUs, 1, 50, Some((100_000, true))),
            "Items 1–50 of 100000+",
            "a capped count says so"
        );
    }

    #[test]
    fn stack_statuses_split_action_and_state() {
        assert_eq!(stack_status(Lang::EnUs, "CREATE_COMPLETE"), "Create Complete");
        assert_eq!(stack_status(Lang::EnUs, "UPDATE_IN_PROGRESS"), "Update In Progress");
        assert_eq!(stack_status(Lang::EsEs, "ROLLBACK_FAILED"), "Reversión fallida");
        assert_eq!(
            stack_status(Lang::EsEs, "ABANDON_COMPLETE"),
            "Abandono completado",
            "masculine noun"
        );
        assert_eq!(
            stack_status(Lang::EnUs, "WEIRD_STATE"),
            "Weird state",
            "unknown: title case"
        );
    }

    #[test]
    fn visibility_and_policies_read_like_horizon() {
        assert_eq!(image_visibility(Lang::EnUs, "public"), "Public");
        assert_eq!(image_visibility(Lang::EsEs, "shared"), "Compartida");
        assert_eq!(image_visibility(Lang::EnUs, "odd"), "odd");
        assert_eq!(
            server_group_policy(Lang::EnUs, "soft-anti-affinity"),
            "Soft Anti Affinity"
        );
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
        // An empty `f64` sum is `-0.0`: zero usage must not read `-0.00`.
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
        assert_eq!(
            both(1234.5, 2),
            ("1,234.50".into(), "1234,50".into()),
            "Spanish leaves four digits ungrouped"
        );
        assert_eq!(both(12345.5, 2), ("12,345.50".into(), "12.345,50".into()));
        assert_eq!(both(999.99, 2), ("999.99".into(), "999,99".into()));
        assert_eq!(
            both(-1234567.0, 0),
            ("-1,234,567".into(), "-1.234.567".into()),
            "the sign stays in front"
        );
        assert_eq!(both(-0.0, 2), ("0.00".into(), "0,00".into()), "never -0.00");
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
    fn an_upload_rate_rounds_the_time_left_up() {
        let mb = 1024.0 * 1024.0;
        assert_eq!(upload_rate(Lang::EnUs, 14.0 * mb, 41.2), "14 MB/s · about 42 s left");
        assert_eq!(upload_rate(Lang::EnUs, 14.0 * mb, 61.0), "14 MB/s · about 2 min left");
        assert_eq!(upload_rate(Lang::EsEs, 2.0 * mb, 7300.0), "2 MB/s · quedan unos 2 h");
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
