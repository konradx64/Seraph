<script>
  import { onMount } from 'svelte';

  // State variables using Svelte 5 runes
  let status = $state("Disconnected");
  let routes = $state([]);
  let logs = $state([]);
  let alertMsg = $state(null);
  let alertSuccess = $state(true);

  // Form inputs
  let newKey = $state("");
  let newUpstream = $state("");
  let newTls = $state("Auto");
  let newTunnel = $state("");

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

  function connectSSE() {
    status = "Connecting...";
    const loc = window.location;
    const proto = loc.protocol;
    const sseUrl = `${proto}//${loc.host}/api/events`;
    
    eventSource = new EventSource(sseUrl);
    
    eventSource.onopen = () => {
      status = "Connected";
      logs = [...logs, { time: new Date().toLocaleTimeString(), text: "Connected to gateway API" }];
      fetchRoutes();
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
  });

  async function addRoute() {
    if (!newKey || !newUpstream) return;
    
    const payload = {
      key: newKey,
      upstream: newUpstream,
      tls: newTls,
      tunnel: newTunnel ? newTunnel : null
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

    // Reset inputs
    newKey = "";
    newUpstream = "";
    newTls = "Auto";
    newTunnel = "";
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

  // Formatting key display (hostname + path_prefix)
  function formatRouteKey(route) {
    return `${route.hostname}${route.path_prefix || ""}`;
  }
</script>

<main class="min-h-screen bg-base-200 text-base-content p-6">
  <!-- Navbar -->
  <div class="navbar bg-base-100 rounded-box shadow-md mb-6 px-6">
    <div class="flex-1">
      <a href="/" class="text-xl font-bold tracking-wider flex items-center gap-2">
        <span class="text-primary font-black">🌼 Seraph</span>
        <span class="text-sm font-semibold opacity-50">Gateway Dashboard</span>
      </a>
    </div>
    <div class="flex-none gap-4">
      <div class="badge badge-lg gap-2 font-bold 
        {status === 'Connected' ? 'badge-success' : 'badge-warning'}">
        <span class="w-2 h-2 rounded-full bg-current animate-ping"></span>
        {status}
      </div>
    </div>
  </div>

  {#if alertMsg}
    <div class="alert {alertSuccess ? 'alert-success' : 'alert-error'} shadow-lg mb-6">
      <div>
        <span>{alertMsg}</span>
      </div>
    </div>
  {/if}

  <!-- Stats Grid -->
  <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-6">
    <div class="stat bg-base-100 rounded-box shadow">
      <div class="stat-title">HTTP Proxy Port</div>
      <div class="stat-value text-primary">8080</div>
      <div class="stat-desc">Listen Address: 0.0.0.0</div>
    </div>
    <div class="stat bg-base-100 rounded-box shadow">
      <div class="stat-title">HTTPS Proxy Port</div>
      <div class="stat-value text-secondary">8443</div>
      <div class="stat-desc">TLS/SNI Resolution: Dynamic</div>
    </div>
    <div class="stat bg-base-100 rounded-box shadow">
      <div class="stat-title">Total Active Routes</div>
      <div class="stat-value">{routes.length}</div>
      <div class="stat-desc">Dynamic Config: config.toml</div>
    </div>
  </div>

  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
    <!-- Routes List -->
    <div class="card lg:col-span-2 bg-base-100 shadow-md">
      <div class="card-body">
        <h2 class="card-title text-xl mb-4">Active Proxy Routes</h2>
        <div class="overflow-x-auto">
          <table class="table w-full">
            <thead>
              <tr>
                <th>Hostname / Prefix</th>
                <th>Upstream Target</th>
                <th>TLS Mode</th>
                <th>Tunnel</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {#each routes as route}
                <tr class="hover">
                  <td class="font-mono text-sm">{formatRouteKey(route)}</td>
                  <td class="font-mono text-sm">{route.upstream}</td>
                  <td>
                    <span class="badge {route.tls === 'Auto' ? 'badge-primary' : 'badge-neutral'}">
                      {route.tls}
                    </span>
                  </td>
                  <td>
                    {#if route.tunnel}
                      <span class="badge badge-secondary">{route.tunnel}</span>
                    {:else}
                      <span class="opacity-30">—</span>
                    {/if}
                  </td>
                  <td>
                    <button class="btn btn-error btn-xs btn-outline" onclick={() => deleteRoute(formatRouteKey(route))}>
                      Delete
                    </button>
                  </td>
                </tr>
              {:else}
                <tr>
                  <td colspan="5" class="text-center py-6 opacity-50">No routes configured</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <!-- Right Column: Add Route & Event Log -->
    <div class="space-y-6">
      <!-- Add Route Form -->
      <div class="card bg-base-100 shadow-md">
        <div class="card-body">
          <h2 class="card-title text-xl mb-4">Add New Route</h2>
          <form onsubmit={(e) => { e.preventDefault(); addRoute(); }} class="space-y-4">
            <div class="form-control">
              <label class="label">
                <span class="label-text">Hostname / Path Key</span>
              </label>
              <input type="text" placeholder="e.g. app.localhost/api" class="input input-bordered w-full" bind:value={newKey} required />
            </div>

            <div class="form-control">
              <label class="label">
                <span class="label-text">Upstream Address</span>
              </label>
              <input type="text" placeholder="e.g. 127.0.0.1:4000 or http://..." class="input input-bordered w-full" bind:value={newUpstream} required />
            </div>

            <div class="grid grid-cols-2 gap-4">
              <div class="form-control">
                <label class="label">
                  <span class="label-text">TLS Mode</span>
                </label>
                <select class="select select-bordered w-full" bind:value={newTls}>
                  <option value="Auto">Auto (TLS)</option>
                  <option value="Off">Off (HTTP)</option>
                </select>
              </div>

              <div class="form-control">
                <label class="label">
                  <span class="label-text">Tunnel Name (Opt)</span>
                </label>
                <input type="text" placeholder="e.g. my-tunnel" class="input input-bordered w-full" bind:value={newTunnel} />
              </div>
            </div>

            <button type="submit" class="btn btn-primary w-full mt-4">Register Route</button>
          </form>
        </div>
      </div>

      <!-- Real-time Event Log -->
      <div class="card bg-base-100 shadow-md">
        <div class="card-body">
          <h2 class="card-title text-xl mb-2">Live Gateway Log</h2>
          <div class="bg-neutral text-neutral-content font-mono text-xs p-4 rounded-box h-48 overflow-y-auto space-y-1">
            {#each logs as log}
              <div>
                <span class="text-neutral-content/40">[{log.time}]</span> {log.text}
              </div>
            {:else}
              <div class="text-neutral-content/40">Waiting for events...</div>
            {/each}
          </div>
        </div>
      </div>
    </div>
  </div>
</main>
