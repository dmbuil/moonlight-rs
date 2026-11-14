//! The root component: the HTML shell rendered on the server and the router with every page's
//! route (Horizon's URL layout, so bookmarks map across). A new page gets its `Route` here and,
//! if it is a panel, a menu entry in `components/nav.rs` (AGENTS.md §4).

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Outlet, ParentRoute, Redirect, Route, Router, Routes};
use leptos_router::{SsrMode, path};

use crate::components::layout::Layout;
use crate::i18n::{I18n, K, initial_lang, use_i18n};
use crate::pages::admin::*;
use crate::pages::alarms::{AlarmDetailPage, AlarmsPage};
use crate::pages::details::*;
use crate::pages::firewall::{FirewallGroupDetailPage, FirewallPolicyDetailPage, FirewallRuleDetailPage};
use crate::pages::floating_ip::FloatingIpDetailPage;
use crate::pages::heat_catalog::{
    ResourceTypeDetailPage, ResourceTypesPage, TemplateVersionDetailPage, TemplateVersionsPage,
};
use crate::pages::identity::*;
use crate::pages::instance_detail::InstanceDetailPage;
use crate::pages::instances::InstancesPage;
use crate::pages::key_manager::{ContainerDetailPage, OrderDetailPage, SecretDetailPage};
use crate::pages::lists::*;
use crate::pages::load_balancers::LoadBalancerDetailPage;
use crate::pages::login::LoginPage;
use crate::pages::overview::OverviewPage;
use crate::pages::qos::QosPolicyDetailPage;
use crate::pages::quotas::{IdentityProjectQuotasPage, QuotasPage};
use crate::pages::stack_launch::{StackChangePage, StackLaunchPage};
use crate::pages::stacks::{StackDetailPage, StacksPage};
use crate::pages::template_generator::TemplateGeneratorPage;
use crate::pages::topology::NetworkTopologyPage;
use crate::pages::trunks::TrunkDetailPage;
use crate::pages::volume_groups::{GroupSnapshotDetailPage, VolumeGroupDetailPage};
use crate::theme::{ThemeCtx, initial_theme};

/// The HTML document the server renders around [`App`]. `lang` and `data-theme` come from the
/// request's cookies (or `Accept-Language` and `default_theme`) so the first paint is already in
/// the user's language and theme, before hydration.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    let lang = initial_lang();
    let theme = initial_theme();
    view! {
        <!DOCTYPE html>
        <html lang=lang.tag() data-theme=theme.attr()>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="color-scheme" content="light dark"/>
                <link rel="icon" href="/favicon.svg" type="image/svg+xml"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

/// The application: provides the language, theme and `<title>` contexts and declares the routes.
/// Every route renders with `SsrMode::Async`, so the server waits for the page's data and can
/// answer an expired session with a redirect instead of a half-rendered page.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_context(I18n(RwSignal::new(initial_lang())));
    provide_context(ThemeCtx(RwSignal::new(initial_theme())));
    // SSR waits for data so redirects (expired session) and full HTML work.
    let ssr = SsrMode::Async;

    view! {
        <Stylesheet id="leptos" href="/pkg/moonlight.css"/>
        <Title formatter=|text: String| if text.is_empty() { "Moonlight".to_string() } else { format!("{text} · Moonlight") }/>
        <Router>
            <Routes fallback=NotFound>
                <Route path=path!("/login") view=LoginPage ssr=ssr.clone()/>
                <Route path=path!("/") view=|| view! { <Redirect path="/project"/> } ssr=ssr.clone()/>
                // One `Layout` for the three dashboards: switching between them keeps
                // the header mounted and the session info loaded, instead of rebuilding
                // both (the screen blinked while it was fetched again).
                <ParentRoute path=path!("") view=Layout ssr=ssr.clone()>
                    <ParentRoute path=path!("/project") view=Outlet>
                        <Route path=path!("") view=OverviewPage/>
                        <Route path=path!("quotas") view=QuotasPage/>
                        <Route path=path!("instances") view=InstancesPage/>
                        <Route path=path!("instances/:id") view=InstanceDetailPage/>
                        <Route path=path!("images") view=ImagesPage/>
                        <Route path=path!("images/:id") view=ImageDetailPage/>
                        <Route path=path!("key_pairs") view=KeyPairsPage/>
                        <Route path=path!("key_pairs/:name") view=KeyPairDetailPage/>
                        <Route path=path!("server_groups") view=ServerGroupsPage/>
                        <Route path=path!("server_groups/:id") view=ServerGroupDetailPage/>
                        <Route path=path!("volumes") view=VolumesPage/>
                        <Route path=path!("volumes/:id") view=VolumeDetailPage/>
                        <Route path=path!("snapshots") view=SnapshotsPage/>
                        <Route path=path!("snapshots/:id") view=SnapshotDetailPage/>
                        <Route path=path!("backups") view=BackupsPage/>
                        <Route path=path!("backups/:id") view=BackupDetailPage/>
                        <Route path=path!("volume_groups") view=VolumeGroupsPage/>
                        <Route path=path!("volume_groups/:id") view=VolumeGroupDetailPage/>
                        <Route path=path!("vg_snapshots") view=GroupSnapshotsPage/>
                        <Route path=path!("vg_snapshots/:id") view=GroupSnapshotDetailPage/>
                        <Route path=path!("network_topology") view=NetworkTopologyPage/>
                        <Route path=path!("networks") view=NetworksPage/>
                        <Route path=path!("networks/:id") view=NetworkDetailPage/>
                        <Route path=path!("networks/subnets/:id") view=SubnetDetailPage/>
                        <Route path=path!("networks/ports/:id") view=PortDetailPage/>
                        <Route path=path!("routers") view=RoutersPage/>
                        <Route path=path!("routers/:id") view=RouterDetailPage/>
                        <Route path=path!("security_groups") view=SecurityGroupsPage/>
                        <Route path=path!("security_groups/:id") view=SecurityGroupDetailPage/>
                        <Route path=path!("floating_ips") view=FloatingIpsPage/>
                        <Route path=path!("floating_ips/:id") view=FloatingIpDetailPage/>
                        <Route path=path!("trunks") view=TrunksPage/>
                        <Route path=path!("trunks/:id") view=TrunkDetailPage/>
                        <Route path=path!("firewall") view=FirewallGroupsPage/>
                        <Route path=path!("firewall/groups/:id") view=FirewallGroupDetailPage/>
                        <Route path=path!("firewall/policies") view=FirewallPoliciesPage/>
                        <Route path=path!("firewall/policies/:id") view=FirewallPolicyDetailPage/>
                        <Route path=path!("firewall/rules") view=FirewallRulesPage/>
                        <Route path=path!("firewall/rules/:id") view=FirewallRuleDetailPage/>
                        <Route path=path!("qos") view=QosPoliciesPage/>
                        <Route path=path!("qos/:id") view=QosPolicyDetailPage/>
                        <Route path=path!("load_balancers") view=LoadBalancersPage/>
                        <Route path=path!("load_balancers/:id") view=LoadBalancerDetailPage/>
                        <Route path=path!("key_manager/secrets") view=SecretsPage/>
                        <Route path=path!("key_manager/secrets/:id") view=SecretDetailPage/>
                        <Route path=path!("key_manager/containers") view=ContainersPage/>
                        <Route path=path!("key_manager/containers/:id") view=ContainerDetailPage/>
                        <Route path=path!("key_manager/orders") view=OrdersPage/>
                        <Route path=path!("key_manager/orders/:id") view=OrderDetailPage/>
                        <Route path=path!("stacks") view=StacksPage/>
                        <Route path=path!("stacks/launch") view=StackLaunchPage/>
                        <Route path=path!("stacks/:id") view=StackDetailPage/>
                        <Route path=path!("stacks/:id/change") view=StackChangePage/>
                        <Route path=path!("alarms") view=AlarmsPage/>
                        <Route path=path!("alarms/:id") view=AlarmDetailPage/>
                        <Route path=path!("resource_types") view=ResourceTypesPage/>
                        <Route path=path!("resource_types/:id") view=ResourceTypeDetailPage/>
                        <Route path=path!("template_versions") view=TemplateVersionsPage/>
                        <Route path=path!("template_versions/:id") view=TemplateVersionDetailPage/>
                        <Route path=path!("template_generator") view=TemplateGeneratorPage/>
                        <Route path=path!("api_access") view=ApiAccessPage/>
                    </ParentRoute>
                    // Horizon's Admin dashboard, Horizon's URLs; `/admin` is
                    // its Overview.
                    <ParentRoute path=path!("/admin") view=Outlet>
                        <Route path=path!("") view=AdminOverviewPage/>
                        <Route path=path!("hypervisors") view=HypervisorsPage/>
                        <Route path=path!("aggregates") view=AggregatesPage/>
                        <Route path=path!("instances") view=AdminInstancesPage/>
                        <Route path=path!("flavors") view=AdminFlavorsPage/>
                        <Route path=path!("images") view=AdminImagesPage/>
                        <Route path=path!("volumes") view=AdminVolumesPage/>
                        <Route path=path!("snapshots") view=AdminSnapshotsPage/>
                        <Route path=path!("volume_types") view=VolumeTypesPage/>
                        <Route path=path!("volume_groups") view=AdminVolumeGroupsPage/>
                        <Route path=path!("vg_snapshots") view=AdminGroupSnapshotsPage/>
                        <Route path=path!("group_types") view=AdminGroupTypesPage/>
                        <Route path=path!("networks") view=AdminNetworksPage/>
                        <Route path=path!("routers") view=AdminRoutersPage/>
                        <Route path=path!("floating_ips") view=AdminFloatingIpsPage/>
                        <Route path=path!("trunks") view=AdminTrunksPage/>
                        <Route path=path!("rbac_policies") view=AdminRbacPoliciesPage/>
                        <Route path=path!("defaults") view=DefaultQuotasPage/>
                        <Route path=path!("metadata_defs") view=MetadefsPage/>
                        <Route path=path!("metadata_defs/:id") view=MetadefDetailPage/>
                        <Route path=path!("info") view=SystemInfoPage/>
                    </ParentRoute>
                    // Horizon's Identity dashboard; `/identity` opens Projects like Horizon's.
                    <ParentRoute path=path!("/identity") view=Outlet>
                        <Route path=path!("") view=|| view! { <Redirect path="/identity/projects"/> }/>
                        <Route path=path!("domains") view=IdentityDomainsPage/>
                        <Route path=path!("projects") view=IdentityProjectsPage/>
                        <Route path=path!("projects/:id") view=IdentityProjectDetailPage/>
                        <Route path=path!("projects/:id/quotas") view=IdentityProjectQuotasPage/>
                        <Route path=path!("users") view=IdentityUsersPage/>
                        <Route path=path!("users/:id") view=IdentityUserDetailPage/>
                        <Route path=path!("groups") view=IdentityGroupsPage/>
                        <Route path=path!("groups/:id") view=IdentityGroupDetailPage/>
                        <Route path=path!("roles") view=IdentityRolesPage/>
                        <Route path=path!("application_credentials") view=AppCredentialsPage/>
                        <Route path=path!("application_credentials/:id") view=AppCredentialDetailPage/>
                    </ParentRoute>
                </ParentRoute>
            </Routes>
        </Router>
    }
}

/// Fallback for a URL no route matches: a 404 with a link back to the project overview.
#[component]
fn NotFound() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <main class="content">
            <h1>"404"</h1>
            <p><a href="/project">{move || i18n.t(K::Project)}</a></p>
        </main>
    }
}
