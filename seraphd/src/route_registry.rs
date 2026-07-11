use super::route::Route;

#[derive(Debug, Clone)]
pub struct RouteRegistry {
    routes: Vec<Route>,
}

impl RouteRegistry {
    pub fn new(routes: Vec<Route>) -> Self {
        Self { routes }
    }

    pub fn match_route(&self, host: &str, path: &str) -> Option<Route> {
        self.routes
            .iter()
            .filter(|route| route.matches(host, path))
            .max_by_key(|route| route.path_prefix.as_ref().map(|p| p.len()).unwrap_or(0))
            .cloned()
    }

    pub fn all(&self) -> &[Route] {
        &self.routes
    }

    pub fn register(&mut self, route: Route) {
        self.routes
            .retain(|r| !(r.hostname == route.hostname && r.path_prefix == route.path_prefix));
        self.routes.push(route);
    }

    pub fn remove(&mut self, hostname: &str, path_prefix: Option<&str>) -> bool {
        let original_len = self.routes.len();
        self.routes
            .retain(|r| !(r.hostname == hostname && r.path_prefix.as_deref() == path_prefix));
        self.routes.len() < original_len
    }
}
