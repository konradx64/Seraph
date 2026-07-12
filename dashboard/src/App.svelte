<script>
  import { onMount } from 'svelte';
  import Header from './components/Header.svelte';
  import OverviewView from './components/OverviewView.svelte';
  import RoutesView from './components/RoutesView.svelte';
  import CertsView from './components/CertsView.svelte';
  import LogsView from './components/LogsView.svelte';
  import SettingsView from './components/SettingsView.svelte';
  import { CheckCircle2 } from '@lucide/svelte';

  // Shared state variables
  let status = $state("Disconnected");
  let activeTab = $state("overview");
  let routes = $state([]);
  let certs = $state([]);
  let logs = $state([]);
  let alertMsg = $state(null);
  let alertSuccess = $state(true);

  let eventSource = null;

  async function fetchRoutes() {
    try {
      const res = await fetch("/api/routes");
      if (res.ok) {
        routes = await res.json();
      }
    } catch (err) {
      console.error("Failed to fetch routes:", err);
    }
  }

  async function fetchCerts() {
    try {
      const res = await fetch("/api/certs");
      if (res.ok) {
        certs = await res.json();
      }
    } catch (err) {
      console.error("Failed to fetch certs:", err);
    }
  }

  function connectSSE() {
    status = "Connecting...";
    const loc = window.location;
    const proto = loc.protocol;
    const sseUrl = `${proto}//${loc.host}/api/events`;
    
    eventSource = new EventSource(sseUrl);
    
    eventSource.onopen = () => {
      status = "Connected";
      logs = [{ time: new Date().toLocaleTimeString(), text: "SSE Event Stream established." }, ...logs];
      fetchRoutes();
      fetchCerts();
    };
    
    eventSource.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      let logText = "";
      
      if (msg.type === "RequestHit") {
        logText = `Proxy: ${msg.host} ${msg.method} ${msg.path} -> ${msg.upstream}`;
      } else if (msg.type === "RequestMiss") {
        logText = `Proxy 404: ${msg.host} ${msg.method} ${msg.path} (No route)`;
      } else if (msg.type === "RouteAdded") {
        logText = `Route for ${msg.key} was added`;
        fetchRoutes();
      } else if (msg.type === "RouteDeleted") {
        logText = `Route for ${msg.key} was deleted`;
        fetchRoutes();
      } else if (msg.type === "CertRegistered") {
        logText = `Certificate registered successfully for ${msg.sni}`;
        fetchCerts();
      } else {
        logText = JSON.stringify(msg);
      }
      
      logs = [{ time: new Date().toLocaleTimeString(), text: logText }, ...logs];
    };

    eventSource.onerror = (err) => {
      console.error("SSE connection error:", err);
      status = "Disconnected";
      eventSource.close();
      setTimeout(connectSSE, 3000);
    };
  }

  onMount(() => {
    connectSSE();
    fetchRoutes();
    fetchCerts();
  });

  async function addRoute(key, upstream, tls, tunnel) {
    const payload = {
      key,
      upstream,
      tls,
      tunnel: tunnel ? tunnel : null
    };

    try {
      const res = await fetch("/api/routes", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      const result = await res.json();
      alertMsg = result.message;
      alertSuccess = result.success;
      setTimeout(() => { alertMsg = null; }, 5000);
      if (res.ok && result.success) {
        fetchRoutes();
      }
    } catch (err) {
      console.error("Failed to add route:", err);
      alertMsg = "Failed to connect to gateway API";
      alertSuccess = false;
      setTimeout(() => { alertMsg = null; }, 5000);
    }
  }

  async function deleteRoute(key) {
    try {
      const res = await fetch(`/api/routes?key=${encodeURIComponent(key)}`, {
        method: "DELETE"
      });
      const result = await res.json();
      alertMsg = result.message;
      alertSuccess = result.success;
      setTimeout(() => { alertMsg = null; }, 5000);
      if (res.ok && result.success) {
        fetchRoutes();
      }
    } catch (err) {
      console.error("Failed to delete route:", err);
    }
  }

  async function registerCert(sni, certPem, keyPem) {
    const payload = {
      sni,
      cert_pem: certPem,
      key_pem: keyPem
    };

    try {
      const res = await fetch("/api/certs", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      const result = await res.json();
      alertMsg = result.message;
      alertSuccess = result.success;
      setTimeout(() => { alertMsg = null; }, 5000);
      if (res.ok && result.success) {
        fetchCerts();
      }
    } catch (err) {
      console.error("Failed to register certificate:", err);
      alertMsg = "Failed to connect to gateway API";
      alertSuccess = false;
      setTimeout(() => { alertMsg = null; }, 5000);
    }
  }

  async function refreshCert(sni) {
    try {
      const res = await fetch("/api/certs/refresh", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sni })
      });
      const result = await res.json();
      alertMsg = result.message;
      alertSuccess = result.success;
      setTimeout(() => { alertMsg = null; }, 5000);
    } catch (err) {
      console.error("Failed to trigger cert refresh:", err);
      alertMsg = "Failed to connect to gateway API";
      alertSuccess = false;
      setTimeout(() => { alertMsg = null; }, 5000);
    }
  }
</script>

<main class="min-h-screen bg-slate-50 text-slate-800" data-theme="light">
  <!-- Sticky Header Bar -->
  <Header {status} bind:activeTab={activeTab} />

  <!-- Page Body -->
  <div class="max-w-7xl mx-auto p-6 space-y-6">
    {#if alertMsg}
      <div class="alert shadow-xs border 
        {alertSuccess ? 'bg-emerald-50 text-emerald-800 border-emerald-200' : 'bg-rose-50 text-rose-800 border-rose-200'}">
        <div class="flex items-center gap-2">
          {#if alertSuccess}
            <CheckCircle2 class="w-4 h-4 text-emerald-600" />
          {/if}
          <span class="font-semibold text-sm">{alertMsg}</span>
        </div>
      </div>
    {/if}

    <!-- Tab Switching Render -->
    {#if activeTab === 'overview'}
      <OverviewView {routes} {certs} {logs} {status} onSwitchTab={(tab) => activeTab = tab} />
    {:else if activeTab === 'routes'}
      <RoutesView {routes} onAdd={addRoute} onDelete={deleteRoute} />
    {:else if activeTab === 'certs'}
      <CertsView {certs} onRegister={registerCert} onRefresh={refreshCert} />
    {:else if activeTab === 'logs'}
      <LogsView bind:logs={logs} />
    {:else if activeTab === 'settings'}
      <SettingsView />
    {/if}
  </div>
</main>
