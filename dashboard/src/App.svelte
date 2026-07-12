<script>
  import { onMount } from 'svelte';
  import logo from './assets/logo.png';
  import { 
    CheckCircle2, 
    Route, 
    ShieldCheck, 
    Trash2, 
    Plus, 
    Zap, 
    RefreshCw, 
    Activity, 
    Search,
    Upload,
    Terminal,
    X,
    Edit,
    Lock,
    Unlock,
    ChevronDown,
    Globe,
    ArrowRight,
    Clock
  } from '@lucide/svelte';

  // Shared state variables
  let status = $state("Disconnected");
  let routes = $state([]);
  let certs = $state([]);
  let logs = $state([]);
  let alertMsg = $state(null);
  let alertSuccess = $state(true);

  // Stats state variable
  let stats = $state({
    total_requests: 0,
    status_2xx: 0,
    status_3xx: 0,
    status_4xx: 0,
    status_5xx: 0,
    rps: 0,
    routes: {}
  });

  let rpsHistory = $state(Array(15).fill(0));

  // Derived values for the sparkline chart
  let maxRps = $derived(Math.max(...rpsHistory, 5));
  let chartPoints = $derived(
    rpsHistory.map((val, index) => {
      const x = (index / (rpsHistory.length - 1 || 1)) * 100;
      const y = 90 - (val / maxRps) * 75; // Keep line between y=15 and y=90 to avoid edge clipping
      return `${x},${y}`;
    }).join(" ")
  );

  // Form local state variables (Add/Edit Route)
  let isEditing = $state(false);
  let originalHostKey = $state("");
  let rHost = $state("");
  let rUpstream = $state("");
  let rUpstreamTls = $state("http");
  let rTls = $state("Enabled");
  let rTunnel = $state(false);
  let rHsts = $state(false);
  let rCorsOrigins = $state("");
  let rForwardIp = $state(true);

  // Form local state variables (Certs)
  let activeCertTab = $state("generate"); // 'generate' | 'acme' | 'upload'
  let cDomain = $state("");
  let cAcmeDomain = $state("");
  let cAcmeEmail = $state("");
  let uSni = $state("");
  let uCertPem = $state("");
  let uKeyPem = $state("");

  let searchQuery = $state("");
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
      logs = [{ time: new Date().toLocaleTimeString(), text: "Event stream connected successfully." }, ...logs];
      fetchRoutes();
      fetchCerts();
    };
    
    eventSource.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      let logText = "";
      
      if (msg.type === "RequestHit") {
        logText = `Proxy Hit: ${msg.host} ${msg.method} ${msg.path} -> ${msg.upstream}`;
      } else if (msg.type === "RequestMiss") {
        logText = `Proxy 404: ${msg.host} ${msg.method} ${msg.path} (No route configured)`;
      } else if (msg.type === "RouteAdded") {
        logText = `Route registered for host ${msg.key}`;
        fetchRoutes();
      } else if (msg.type === "RouteDeleted") {
        logText = `Route for ${msg.key} deleted`;
        fetchRoutes();
      } else if (msg.type === "CertRegistered") {
        logText = `SSL Certificate stored for SNI: ${msg.sni}`;
        fetchCerts();
      } else if (msg.type === "Log") {
        logText = msg.text;
      } else if (msg.type === "StatsUpdate") {
        stats = {
          total_requests: msg.total_requests,
          status_2xx: msg.status_2xx,
          status_3xx: msg.status_3xx,
          status_4xx: msg.status_4xx,
          status_5xx: msg.status_5xx,
          rps: msg.rps || 0,
          routes: msg.routes || {}
        };
        // Keep the last 15 data points for a smooth line chart
        rpsHistory = [...rpsHistory.slice(-14), msg.rps || 0];
        return; // Skip logs list append for stats updates
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

  function startCreateRoute() {
    isEditing = false;
    originalHostKey = "";
    rHost = "";
    rUpstream = "";
    rUpstreamTls = "http";
    rTls = "Enabled";
    rTunnel = false;
    rHsts = false;
    rCorsOrigins = "";
    rForwardIp = true;
    document.getElementById('add_route_modal').showModal();
  }

  function startEditRoute(route) {
    isEditing = true;
    originalHostKey = route.hostname;
    rHost = route.hostname;
    rUpstream = route.upstream;
    rUpstreamTls = route.upstream_tls ? "https" : "http";
    rTunnel = route.tunnel === "true" || route.tunnel === true;
    rTls = route.tls || "Enabled";
    rHsts = route.hsts || false;
    rCorsOrigins = route.cors_origins || "";
    rForwardIp = route.forward_ip !== false;
    document.getElementById('add_route_modal').showModal();
  }

  async function submitRoute() {
    if (!rHost || !rUpstream) return;

    if (isEditing && originalHostKey !== rHost) {
      try {
        await fetch(`/api/routes?key=${encodeURIComponent(originalHostKey)}`, {
          method: "DELETE"
        });
      } catch (err) {
        console.error("Failed to delete old route key:", err);
      }
    }

    const payload = {
      key: rHost,
      upstream: rUpstream,
      tls: rTls,
      tunnel: rTunnel ? "true" : null,
      upstream_tls: rUpstreamTls === "https",
      hsts: rHsts,
      cors_origins: rCorsOrigins || null,
      forward_ip: rForwardIp
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
        document.getElementById('add_route_modal').close();
      }
    } catch (err) {
      console.error("Failed to save route:", err);
      alertMsg = "Failed to connect to gateway API";
      alertSuccess = false;
      setTimeout(() => { alertMsg = null; }, 5000);
    }
  }

  async function requestAcmeCert() {
    if (!cAcmeDomain || !cAcmeEmail) return;
    try {
      const res = await fetch("/api/certs/acme", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sni: cAcmeDomain, email: cAcmeEmail })
      });
      const result = await res.json();
      alertMsg = result.message;
      alertSuccess = result.success;
      setTimeout(() => { alertMsg = null; }, 5000);
      if (res.ok && result.success) {
        fetchCerts();
        cAcmeDomain = "";
        cAcmeEmail = "";
        document.getElementById('add_cert_modal').close();
      }
    } catch (err) {
      console.error("Failed to request Let's Encrypt TLS certificate:", err);
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

  async function registerCert() {
    if (!uSni || !uCertPem || !uKeyPem) return;
    const payload = {
      sni: uSni,
      cert_pem: uCertPem,
      key_pem: uKeyPem
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
        uSni = "";
        uCertPem = "";
        uKeyPem = "";
        document.getElementById('add_cert_modal').close();
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

  async function generateCert() {
    if (!cDomain) return;
    try {
      const res = await fetch("/api/certs/generate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sni: cDomain })
      });
      const result = await res.json();
      alertMsg = result.message;
      alertSuccess = result.success;
      setTimeout(() => { alertMsg = null; }, 5000);
      if (res.ok && result.success) {
        fetchCerts();
        cDomain = "";
        document.getElementById('add_cert_modal').close();
      }
    } catch (err) {
      console.error("Failed to generate certificate:", err);
      alertMsg = "Failed to connect to gateway API";
      alertSuccess = false;
      setTimeout(() => { alertMsg = null; }, 5000);
    }
  }



  // Filtered logs derived state
  let filteredLogs = $derived(
    logs.filter(log => log.text.toLowerCase().includes(searchQuery.toLowerCase()))
  );
</script>

<main class="min-h-screen bg-white text-slate-800 flex flex-col font-sans" data-theme="light">
  <!-- Top Bar -->
  <div class="bg-white border-b border-slate-200 px-8 py-4 flex items-center justify-between sticky top-0 z-40">
    <div class="flex items-center gap-3">
      <img src={logo} alt="Seraph Logo" class="w-8 h-8 object-contain" />
      <div>
        <h1 class="text-base font-black tracking-tight text-slate-900 leading-none">Seraph</h1>
        <span class="text-[11px] font-bold text-slate-400">Reverse Proxy & TLS Coordinator</span>
      </div>
    </div>
    
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-2">
        <span class="relative flex h-2 w-2">
          {#if status === "Connected"}
            <span class="absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
            <span class="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
          {:else if status.startsWith("Connecting")}
            <span class="absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75"></span>
            <span class="relative inline-flex rounded-full h-2 w-2 bg-amber-500"></span>
          {:else}
            <span class="relative inline-flex rounded-full h-2 w-2 bg-rose-500"></span>
          {/if}
        </span>
        <span class="text-xs font-bold text-slate-500 uppercase tracking-wider">{status}</span>
      </div>
    </div>
  </div>

  <div class="flex-1 max-w-[1600px] w-full mx-auto p-8 space-y-8">
    <!-- Action Notifications -->
    {#if alertMsg}
      <div class="alert border py-3.5 px-4 rounded-xl
        {alertSuccess ? 'bg-emerald-50 text-emerald-800 border-emerald-200' : 'bg-rose-50 text-rose-800 border-rose-200'}">
        <div class="flex items-center gap-2">
          {#if alertSuccess}
            <CheckCircle2 class="w-4.5 h-4.5 text-emerald-600" />
          {/if}
          <span class="font-bold text-sm">{alertMsg}</span>
        </div>
      </div>
    {/if}

    <!-- Stat Bar -->
    <div class="grid grid-cols-1 md:grid-cols-4 gap-6">
      <div class="card bg-white border border-slate-200 rounded-xl p-5 flex flex-row items-center justify-between h-[104px]">
        <div>
          <span class="text-xs font-extrabold text-slate-400 uppercase tracking-widest">Active Routes</span>
          <p class="text-xl font-black text-slate-800 leading-none mt-1">{routes.length}</p>
        </div>
        <div class="bg-cyan-50 p-2.5 rounded-lg text-cyan-600">
          <Route class="w-5 h-5" />
        </div>
      </div>

      <div class="card bg-white border border-slate-200 rounded-xl p-5 flex flex-row items-center justify-between h-[104px]">
        <div>
          <span class="text-xs font-extrabold text-slate-400 uppercase tracking-widest">Certificates</span>
          <p class="text-xl font-black text-slate-800 leading-none mt-1">{certs.length}</p>
        </div>
        <div class="bg-violet-50 p-2.5 rounded-lg text-violet-600">
          <ShieldCheck class="w-5 h-5" />
        </div>
      </div>

      <div class="card bg-white border border-slate-200 rounded-xl p-5 flex flex-col justify-between h-[104px]">
        <div class="flex items-center justify-between">
          <div>
            <span class="text-xs font-extrabold text-slate-400 uppercase tracking-widest">Total Requests</span>
            <p class="text-xl font-black text-slate-800 leading-none mt-1">{stats.total_requests}</p>
          </div>
          <div class="bg-emerald-50 p-2 rounded-lg text-emerald-600">
            <Activity class="w-4 h-4" />
          </div>
        </div>
        
        <div class="h-8 overflow-hidden flex items-end mt-1">
          {#if rpsHistory.length > 1}
            <svg class="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
              <polygon points="0,100 {chartPoints} 100,100" class="fill-cyan-500/10" />
              <polyline points={chartPoints} class="stroke-cyan-500 stroke-[3] fill-none" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          {:else}
            <span class="text-[9px] text-slate-400">Awaiting stream...</span>
          {/if}
        </div>
      </div>

      <div class="card bg-white border border-slate-200 rounded-xl p-5 flex flex-row items-center justify-between h-[104px]">
        <div>
          <span class="text-xs font-extrabold text-slate-400 uppercase tracking-widest">Gateway Speed</span>
          <p class="text-xl font-black text-cyan-600 leading-none mt-1">{stats.rps} <span class="text-xs font-bold text-slate-400">req/sec</span></p>
        </div>
        <div class="bg-cyan-50 p-2.5 rounded-lg text-cyan-600">
          <Zap class="w-5 h-5" />
        </div>
      </div>
    </div>

    <!-- Main Workspace Grid -->
    <div class="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
      
      <!-- LEFT WORKSPACE: CONFIGURATION PANELS (8/12 width) -->
      <div class="lg:col-span-8 space-y-8">
        
        <!-- Routes List -->
        <div class="card bg-white border border-slate-200 rounded-xl">
          <div class="card-body p-6">
            <div class="flex items-center justify-between mb-4">
              <h2 class="text-slate-900 font-bold text-sm uppercase tracking-wider flex items-center gap-2">
                <Route class="w-4.5 h-4.5 text-cyan-500" />
                Routes
              </h2>
              <button class="btn btn-xs bg-cyan-50 hover:bg-cyan-100 text-cyan-600 border-none rounded-md px-3 py-2 flex items-center gap-1.5 font-extrabold text-xs" 
                onclick={startCreateRoute}>
                <Plus class="w-4 h-4" />
                Add Route
              </button>
            </div>
            
            <div class="overflow-x-auto">
              <table class="table table-sm w-full">
                <thead>
                  <tr class="text-slate-400 border-slate-200 text-xs uppercase">
                    <th><span class="flex items-center gap-1.5"><Globe class="w-3.5 h-3.5 text-slate-400" /> Hostname</span></th>
                    <th><span class="flex items-center gap-1.5"><ArrowRight class="w-3.5 h-3.5 text-slate-400" /> Upstream Path</span></th>
                    <th><span class="flex items-center gap-1.5"><Activity class="w-3.5 h-3.5 text-slate-400" /> Status</span></th>
                    <th><span class="flex items-center gap-1.5"><Zap class="w-3.5 h-3.5 text-slate-400" /> Requests</span></th>
                    <th><span class="flex items-center gap-1.5"><Clock class="w-3.5 h-3.5 text-slate-400" /> Latency</span></th>
                    <th><span class="flex items-center gap-1.5"><Lock class="w-3.5 h-3.5 text-slate-400" /> TLS</span></th>
                    <th class="w-20 text-right"></th>
                  </tr>
                </thead>
                <tbody class="text-slate-700 text-sm">
                  {#each routes as route (route.hostname)}
                    <tr class="border-slate-200 hover:bg-slate-50">
                      <td class="font-mono font-bold text-slate-800">{route.hostname}</td>
                      <td class="font-mono text-slate-500">{route.upstream_tls ? 'https://' : 'http://'}{route.upstream}</td>
                      
                      <!-- Status (Online/Offline) -->
                      <td>
                        {#if stats.routes[route.hostname]}
                          {#if stats.routes[route.hostname].online}
                            <span class="flex items-center gap-1.5 text-xs text-emerald-600 font-bold">
                              <span class="h-1.5 w-1.5 rounded-full bg-emerald-500"></span>
                              Online
                            </span>
                          {:else}
                            <span class="flex items-center gap-1.5 text-xs text-rose-600 font-bold">
                              <span class="h-1.5 w-1.5 rounded-full bg-rose-500"></span>
                              Offline
                            </span>
                          {/if}
                        {:else}
                          <!-- Default to Online if no traffic occurred yet -->
                          <span class="flex items-center gap-1.5 text-xs text-emerald-600 font-bold">
                            <span class="h-1.5 w-1.5 rounded-full bg-emerald-500"></span>
                            Online
                          </span>
                        {/if}
                      </td>

                      <!-- Requests -->
                      <td class="font-mono text-xs font-bold text-slate-600">
                        {stats.routes[route.hostname]?.total_requests || 0}
                      </td>

                      <!-- Latency -->
                      <td class="font-mono text-xs text-slate-600">
                        {stats.routes[route.hostname]?.total_requests > 0 
                          ? `${stats.routes[route.hostname].avg_latency_ms} ms` 
                          : '—'}
                      </td>

                      <td>
                        {#if route.tls === 'Disabled'}
                          <span class="badge badge-xs p-1 text-slate-500 bg-slate-100 border-none font-bold text-[10px]">DISABLED</span>
                        {:else if route.tls === 'Enforced'}
                          <span class="badge badge-xs p-1 text-cyan-700 bg-cyan-50 border-cyan-200 font-bold text-[10px]">ENFORCED</span>
                        {:else}
                          <span class="badge badge-xs p-1 text-emerald-700 bg-emerald-50 border-emerald-200 font-bold text-[10px]">ENABLED</span>
                        {/if}
                      </td>
                      <td class="text-right flex gap-1 justify-end">
                        <button class="btn btn-ghost btn-xs text-cyan-600 hover:bg-cyan-50 rounded-md p-1" onclick={() => startEditRoute(route)}>
                          <Edit class="w-4.5 h-4.5" />
                        </button>
                        <button class="btn btn-ghost btn-xs text-rose-500 hover:bg-rose-50 rounded-md p-1" onclick={() => deleteRoute(route.hostname)}>
                          <Trash2 class="w-4.5 h-4.5" />
                        </button>
                      </td>
                    </tr>
                  {:else}
                    <tr>
                      <td colspan="7" class="text-center py-10 text-slate-400 text-xs">No routes configured. Click "Add Route" to define one.</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <!-- SSL/TLS Domain Registry -->
        <div class="card bg-white border border-slate-200 rounded-xl">
          <div class="card-body p-6">
            <div class="flex items-center justify-between mb-4">
              <h2 class="text-slate-900 font-bold text-sm uppercase tracking-wider flex items-center gap-2">
                <ShieldCheck class="w-4.5 h-4.5 text-cyan-500" />
                TLS Certificates
              </h2>
              <button class="btn btn-xs bg-cyan-50 hover:bg-cyan-100 text-cyan-600 border-none rounded-md px-3 py-2 flex items-center gap-1.5 font-extrabold text-xs" 
                onclick={() => document.getElementById('add_cert_modal').showModal()}>
                <Plus class="w-4 h-4" />
                Configure Cert
              </button>
            </div>
            
            <div class="overflow-x-auto">
              <table class="table table-sm w-full">
                <thead>
                  <tr class="text-slate-400 border-slate-200 text-xs uppercase">
                    <th>Domain</th>
                    <th>Status</th>
                    <th class="w-20 text-right"></th>
                  </tr>
                </thead>
                <tbody class="text-slate-700 text-sm">
                  {#each certs as cert (cert)}
                    <tr class="border-slate-200 hover:bg-slate-50">
                      <td class="font-mono font-bold text-slate-800">{cert}</td>
                      <td>
                        <span class="badge badge-xs p-1 text-emerald-700 bg-emerald-50 border-emerald-200/60 font-bold text-[10px]">Active</span>
                      </td>
                      <td class="text-right">
                        <button class="btn btn-ghost btn-xs text-cyan-600 hover:bg-cyan-50 font-black text-xs flex items-center gap-1.5 rounded-md px-2 py-1" onclick={() => refreshCert(cert)}>
                          <RefreshCw class="w-3.5 h-3.5" />
                          Renew
                        </button>
                      </td>
                    </tr>
                  {:else}
                    <tr>
                      <td colspan="3" class="text-center py-10 text-slate-400 text-xs">No active TLS certificates registered.</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>
        </div>

      </div>

      <!-- RIGHT WORKSPACE: STATS & LOGS (4/12 width) -->
      <div class="lg:col-span-4 space-y-8">
        <!-- Traffic Distribution Progress Bars -->
        <div class="card bg-white border border-slate-200 rounded-xl">
          <div class="card-body p-6">
            <h2 class="text-slate-900 font-bold text-xs uppercase tracking-wider flex items-center gap-2 mb-4">
              <Activity class="w-4.5 h-4.5 text-cyan-500" />
              Traffic Distribution
            </h2>
            <div class="space-y-3.5">
              <div>
                <div class="flex justify-between text-xs font-bold text-slate-500 mb-1">
                  <span>Successful (2xx)</span>
                  <span>{stats.status_2xx}</span>
                </div>
                <progress class="progress progress-success w-full h-1.5" value={stats.status_2xx} max={stats.total_requests || 1}></progress>
              </div>
              
              <div>
                <div class="flex justify-between text-xs font-bold text-slate-500 mb-1">
                  <span>Redirects (3xx)</span>
                  <span>{stats.status_3xx}</span>
                </div>
                <progress class="progress progress-info w-full h-1.5" value={stats.status_3xx} max={stats.total_requests || 1}></progress>
              </div>

              <div>
                <div class="flex justify-between text-xs font-bold text-slate-500 mb-1">
                  <span>Client Errors (4xx)</span>
                  <span>{stats.status_4xx}</span>
                </div>
                <progress class="progress progress-warning w-full h-1.5" value={stats.status_4xx} max={stats.total_requests || 1}></progress>
              </div>

              <div>
                <div class="flex justify-between text-xs font-bold text-slate-500 mb-1">
                  <span>Server Errors (5xx)</span>
                  <span>{stats.status_5xx}</span>
                </div>
                <progress class="progress progress-error w-full h-1.5" value={stats.status_5xx} max={stats.total_requests || 1}></progress>
              </div>
            </div>
          </div>
        </div>

        <!-- Live Proxy Debugger -->
        <div class="card bg-white border border-slate-200 rounded-xl h-[420px] flex flex-col overflow-hidden">
          <!-- Header -->
          <div class="bg-white border-b border-slate-200 px-5 py-4 flex items-center justify-between sticky top-0 z-10">
            <div class="flex items-center gap-2.5">
              <Terminal class="w-4.5 h-4.5 text-cyan-600" />
              <span class="text-xs font-bold text-slate-800 uppercase tracking-wider">Request Logs</span>
            </div>
            <button class="btn btn-ghost btn-xs text-xs text-slate-400 hover:text-slate-600 hover:bg-slate-50 rounded-md px-2.5" onclick={() => logs = []}>
              Clear
            </button>
          </div>

          <!-- Filter Bar -->
          <div class="bg-white px-4 py-2 border-b border-slate-200 flex items-center gap-2">
            <Search class="w-4 h-4 text-slate-400" />
            <input type="text" placeholder="Filter path, host, or logs..." class="bg-transparent text-sm text-slate-600 w-full focus:outline-hidden font-sans" bind:value={searchQuery} />
          </div>

          <!-- Log Stream Contents -->
          <div class="flex-1 overflow-y-auto p-5 space-y-3 font-mono text-[11.5px]">
            {#each filteredLogs as log}
              <div class="flex items-start gap-2 border-b border-slate-200 pb-2 leading-relaxed">
                <span class="text-slate-400 select-none text-[10px] font-bold">{log.time}</span>
                {#if log.text.includes("Proxy 404") || log.text.includes("failed") || log.text.includes("Error")}
                  <span class="text-rose-600 font-bold">{log.text}</span>
                {:else if log.text.includes("Proxy Hit:") || log.text.includes("stored") || log.text.includes("registered")}
                  <span class="text-emerald-600 font-medium">{log.text}</span>
                {:else if log.text.includes("challenge") || log.text.includes("ACME")}
                  <span class="text-amber-600 font-medium">{log.text}</span>
                {:else}
                  <span class="text-slate-600">{log.text}</span>
                {/if}
              </div>
            {:else}
              <div class="h-full flex flex-col items-center justify-center text-slate-400 gap-2">
                <Activity class="w-7 h-7 text-slate-300 animate-pulse" />
                <span class="text-xs uppercase font-bold tracking-widest text-slate-400">Awaiting live traffic...</span>
              </div>
            {/each}
          </div>
        </div>
      </div>

    </div>
  </div>

  <!-- DIALOG MODAL 1: ADD/EDIT ROUTE OVERLAY -->
  <dialog id="add_route_modal" class="modal">
    <div class="modal-box bg-white border border-slate-200 rounded-xl max-w-sm p-6 relative">
      <button class="btn btn-xs btn-circle btn-ghost text-slate-400 hover:text-slate-700 absolute right-4 top-4" onclick={() => document.getElementById('add_route_modal').close()}>
        <X class="w-4 h-4" />
      </button>
      
      <h3 class="font-bold text-xs uppercase tracking-wider text-slate-800 mb-4 flex items-center gap-2">
        <Route class="w-4.5 h-4.5 text-cyan-500" />
        {isEditing ? 'Edit Proxy Route' : 'Add Proxy Route'}
      </h3>
      
      <form onsubmit={(e) => { e.preventDefault(); submitRoute(); }} class="space-y-4">
        <div class="form-control">
          <label class="label py-0.5" for="m-route-host"><span class="label-text text-xs font-bold text-slate-500">Hostname</span></label>
          <input id="m-route-host" type="text" placeholder="e.g. app.localhost" class="input input-bordered input-sm rounded-md w-full focus:border-cyan-500 focus:outline-hidden text-xs" bind:value={rHost} required />
        </div>
        
        <div class="form-control">
          <label class="label py-0.5" for="m-route-upstream"><span class="label-text text-xs font-bold text-slate-500">Upstream Destination (IP:Port)</span></label>
          <div class="join w-full">
            <select class="select select-bordered select-sm rounded-l-md focus:border-cyan-500 focus:outline-hidden text-xs join-item" bind:value={rUpstreamTls}>
              <option value="http">http://</option>
              <option value="https">https://</option>
            </select>
            <input id="m-route-upstream" type="text" placeholder="e.g. 127.0.0.1:8080" class="input input-bordered input-sm rounded-r-md w-full focus:border-cyan-500 focus:outline-hidden text-xs join-item" bind:value={rUpstream} required />
          </div>
        </div>

        <div class="form-control relative">
          <label class="label py-0.5" for="m-route-tls"><span class="label-text text-xs font-bold text-slate-500">TLS</span></label>
          
          <div class="dropdown w-full">
            <div tabindex="0" role="button" class="btn btn-sm btn-outline border-slate-200 hover:border-slate-300 w-full justify-between font-normal text-xs rounded-md bg-white text-slate-700 hover:bg-slate-50 hover:text-slate-800">
              <span class="flex items-center gap-2">
                {#if rTls === 'Disabled'}
                  <Unlock class="w-4 h-4 text-slate-400" />
                  <span>Disabled (HTTP Only)</span>
                {:else if rTls === 'Enforced'}
                  <ShieldCheck class="w-4 h-4 text-cyan-500" />
                  <span>Enforced (Redirect HTTP to HTTPS) <span class="text-[10px] text-cyan-600 font-bold ml-1">(Recommended)</span></span>
                {:else}
                  <Lock class="w-4 h-4 text-emerald-500" />
                  <span>Enabled (HTTP & HTTPS)</span>
                {/if}
              </span>
              <ChevronDown class="w-4 h-4 text-slate-400" />
            </div>
            
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <ul tabindex="0" class="dropdown-content menu p-1 shadow-lg bg-white border border-slate-200 rounded-lg w-full mt-1 z-50 text-xs text-slate-600">
              <li>
                <button type="button" class="flex items-center gap-2.5 p-2 rounded-md hover:bg-slate-50 text-left w-full
                  {rTls === 'Disabled' ? 'bg-slate-50 text-slate-900 font-bold' : ''}"
                  onclick={() => { rTls = 'Disabled'; document.activeElement.blur(); }}>
                  <Unlock class="w-4 h-4 text-slate-400" />
                  <div class="flex flex-col">
                    <span class="font-bold text-slate-700">Disabled</span>
                    <span class="text-[10px] text-slate-400">Plain HTTP only on port 8080.</span>
                  </div>
                </button>
              </li>
              <li>
                <button type="button" class="flex items-center gap-2.5 p-2 rounded-md hover:bg-slate-50 text-left w-full
                  {rTls === 'Enabled' ? 'bg-slate-50 text-slate-900 font-bold' : ''}"
                  onclick={() => { rTls = 'Enabled'; document.activeElement.blur(); }}>
                  <Lock class="w-4 h-4 text-emerald-500" />
                  <div class="flex flex-col">
                    <span class="font-bold text-slate-700">Enabled</span>
                    <span class="text-[10px] text-slate-400">Accept HTTP or secure HTTPS.</span>
                  </div>
                </button>
              </li>
              <li>
                <button type="button" class="flex items-center gap-2.5 p-2 rounded-md hover:bg-slate-50 text-left w-full
                  {rTls === 'Enforced' ? 'bg-slate-50 text-slate-900 font-bold' : ''}"
                  onclick={() => { rTls = 'Enforced'; document.activeElement.blur(); }}>
                  <ShieldCheck class="w-4 h-4 text-cyan-500" />
                  <div class="flex flex-col">
                    <div class="flex items-center gap-1.5">
                      <span class="font-bold text-slate-700">Enforced</span>
                      <span class="badge badge-xs bg-cyan-100 text-cyan-700 border-none font-black text-[8px] px-1 py-0.5">RECOMMENDED</span>
                    </div>
                    <span class="text-[10px] text-slate-400">Automatically redirect any plain HTTP requests to HTTPS.</span>
                  </div>
                </button>
              </li>
            </ul>
          </div>
        </div>

        <div class="form-control flex flex-row items-center justify-between bg-white p-2.5 rounded-lg border border-slate-200 mt-2">
          <div class="flex flex-col">
            <span class="text-xs font-bold text-slate-700">Enforce HSTS</span>
            <span class="text-[9px] text-slate-400">Strict browser HTTPS security policy</span>
          </div>
          <input type="checkbox" class="checkbox checkbox-xs checkbox-cyan rounded-md" bind:checked={rHsts} disabled={rTls === 'Disabled'} />
        </div>

        <div class="form-control mt-2">
          <label class="label py-0.5" for="m-route-cors"><span class="label-text text-xs font-bold text-slate-500">CORS Allowed Origins (Optional)</span></label>
          <input id="m-route-cors" type="text" placeholder="e.g. *, http://localhost:3000" class="input input-bordered input-sm rounded-md w-full focus:border-cyan-500 focus:outline-hidden text-xs" bind:value={rCorsOrigins} />
        </div>

        <div class="form-control flex flex-row items-center justify-between bg-white p-2.5 rounded-lg border border-slate-200 mt-2">
          <div class="flex flex-col">
            <span class="text-xs font-bold text-slate-700">Forward Client IP</span>
            <span class="text-[9px] text-slate-400">Inject X-Real-IP into upstream headers</span>
          </div>
          <input type="checkbox" class="checkbox checkbox-xs checkbox-cyan rounded-md" bind:checked={rForwardIp} />
        </div>

        <div class="form-control flex flex-row items-center justify-between bg-white p-2.5 rounded-lg border border-slate-200 mt-2">
          <span class="text-xs font-bold text-slate-500">Tunnel Connection</span>
          <input type="checkbox" class="checkbox checkbox-xs checkbox-cyan rounded-md" bind:checked={rTunnel} />
        </div>

        <button type="submit" class="btn btn-sm w-full bg-cyan-500 hover:bg-cyan-600 border-none rounded-md font-bold text-white mt-4">
          {isEditing ? 'Update Route' : 'Save Routing Configuration'}
        </button>
      </form>
    </div>
    <form method="dialog" class="modal-backdrop bg-slate-900/30">
      <button>close</button>
    </form>
  </dialog>

  <!-- DIALOG MODAL 2: CERTIFICATE CONFIGURATION OVERLAY -->
  <dialog id="add_cert_modal" class="modal">
    <div class="modal-box bg-white border border-slate-200 rounded-xl max-w-sm p-6 relative">
      <button class="btn btn-xs btn-circle btn-ghost text-slate-400 hover:text-slate-700 absolute right-4 top-4" onclick={() => document.getElementById('add_cert_modal').close()}>
        <X class="w-4 h-4" />
      </button>

      <h3 class="font-bold text-xs uppercase tracking-wider text-slate-800 mb-4 flex items-center gap-2">
        <ShieldCheck class="w-4.5 h-4.5 text-cyan-500" />
        Configure TLS
      </h3>

      <!-- Tab Switcher -->
      <div class="flex border border-slate-200 p-0.5 rounded-lg bg-white mb-4">
        <button class="flex-1 text-center py-1 rounded-md text-[10px] font-bold transition-none
          {activeCertTab === 'generate' ? 'bg-slate-100 text-slate-800' : 'text-slate-500 hover:text-slate-800'}"
          onclick={() => activeCertTab = 'generate'}>
          On-the-Fly Gen
        </button>
        <button class="flex-1 text-center py-1 rounded-md text-[10px] font-bold transition-none
          {activeCertTab === 'acme' ? 'bg-slate-100 text-slate-800' : 'text-slate-500 hover:text-slate-800'}"
          onclick={() => activeCertTab = 'acme'}>
          Let's Encrypt
        </button>
        <button class="flex-1 text-center py-1 rounded-md text-[10px] font-bold transition-none
          {activeCertTab === 'upload' ? 'bg-slate-100 text-slate-800' : 'text-slate-500 hover:text-slate-800'}"
          onclick={() => activeCertTab = 'upload'}>
          Upload Custom
        </button>
      </div>

      <!-- Tab A: Generate Certificate -->
      {#if activeCertTab === 'generate'}
        <form onsubmit={(e) => { e.preventDefault(); generateCert(); }} class="space-y-4">
          <p class="text-slate-500 text-xs leading-relaxed">
            Generate an instant, self-signed TLS certificate for development Snis.
          </p>
          <div class="form-control">
            <label class="label py-0.5" for="g-cert-domain"><span class="label-text text-xs font-bold text-slate-500">Domain / SNI Name</span></label>
            <input id="g-cert-domain" type="text" placeholder="e.g. app.local" class="input input-bordered input-sm rounded-md w-full focus:border-cyan-500 focus:outline-hidden text-xs" bind:value={cDomain} required />
          </div>
          <button type="submit" class="btn btn-sm w-full bg-amber-500 hover:bg-amber-600 border-none rounded-md font-bold text-white">
            Generate Certificate
          </button>
        </form>
      <!-- Tab B: Let's Encrypt ACME -->
      {:else if activeCertTab === 'acme'}
        <form onsubmit={(e) => { e.preventDefault(); requestAcmeCert(); }} class="space-y-4">
          <p class="text-slate-500 text-xs leading-relaxed">
            Request an automated TLS certificate from Let's Encrypt.
          </p>
          <div class="form-control">
            <label class="label py-0.5" for="a-cert-domain"><span class="label-text text-xs font-bold text-slate-500">Domain Name (FQDN)</span></label>
            <input id="a-cert-domain" type="text" placeholder="e.g. myapp.domain.com" class="input input-bordered input-sm rounded-md w-full focus:border-cyan-500 focus:outline-hidden text-xs" bind:value={cAcmeDomain} required />
          </div>
          <div class="form-control">
            <label class="label py-0.5" for="a-cert-email"><span class="label-text text-xs font-bold text-slate-500">Contact Email</span></label>
            <input id="a-cert-email" type="email" placeholder="e.g. admin@domain.com" class="input input-bordered input-sm rounded-md w-full focus:border-cyan-500 focus:outline-hidden text-xs" bind:value={cAcmeEmail} required />
          </div>
          <button type="submit" class="btn btn-sm w-full bg-emerald-600 hover:bg-emerald-700 border-none rounded-md font-bold text-white">
            Request TLS Certificate
          </button>
        </form>
      <!-- Tab C: Upload Certificate -->
      {:else}
        <form onsubmit={(e) => { e.preventDefault(); registerCert(); }} class="space-y-3">
          <div class="form-control">
            <label class="label py-0.5" for="u-cert-sni"><span class="label-text text-xs font-bold text-slate-500">Domain / SNI Name</span></label>
            <input id="u-cert-sni" type="text" placeholder="e.g. secure.domain.com" class="input input-bordered input-sm rounded-md w-full focus:border-cyan-500 focus:outline-hidden text-xs" bind:value={uSni} required />
          </div>
          <div class="form-control">
            <label class="label py-0.5" for="u-cert-chain"><span class="label-text text-xs font-bold text-slate-500">Certificate PEM Chain</span></label>
            <textarea id="u-cert-chain" rows="3" placeholder="-----BEGIN CERTIFICATE-----..." class="textarea textarea-bordered textarea-xs font-mono rounded-md focus:border-cyan-500" bind:value={uCertPem} required></textarea>
          </div>
          <div class="form-control">
            <label class="label py-0.5" for="u-cert-key"><span class="label-text text-xs font-bold text-slate-500">Private Key PEM</span></label>
            <textarea id="u-cert-key" rows="3" placeholder="-----BEGIN PRIVATE KEY-----..." class="textarea textarea-bordered textarea-xs font-mono rounded-md focus:border-cyan-500" bind:value={uKeyPem} required></textarea>
          </div>
          <button type="submit" class="btn btn-sm w-full bg-cyan-500 hover:bg-cyan-600 border-none rounded-md font-bold text-white">
            Upload Certificate
          </button>
        </form>
      {/if}
    </div>
    <form method="dialog" class="modal-backdrop bg-slate-900/30">
      <button>close</button>
    </form>
  </dialog>
</main>

<style>
  /* Completely disable all transitions, scaling, and animations to remove any "smoothness" */
  :global(*), :global(*::before), :global(*::after) {
    transition: none !important;
    animation: none !important;
  }
</style>
