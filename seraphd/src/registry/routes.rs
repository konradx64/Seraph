use crate::route::Route;

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
}

