<script>
  import { Globe, Plus, Trash2 } from '@lucide/svelte';

  // Svelte 5 props
  let { routes = [], onAdd, onDelete } = $props();

  // Local form inputs state
  let newKey = $state("");
  let newUpstream = $state("");
  let newTls = $state("Auto");
  let newTunnel = $state("");

  function handleSubmit() {
    if (!newKey || !newUpstream) return;
    onAdd(newKey, newUpstream, newTls, newTunnel);
    
    // Reset local form inputs
    newKey = "";
    newUpstream = "";
    newTls = "Auto";
    newTunnel = "";
  }

  function formatRouteKey(route) {
    return `${route.hostname}${route.path_prefix || ""}`;
  }
</script>

<div class="grid grid-cols-1 lg:grid-cols-3 gap-6 animate-fade-in">
  <!-- Routes List Table -->
  <div class="card lg:col-span-2 bg-white border border-slate-200/80 rounded-xl shadow-xs">
    <div class="card-body p-6">
      <h2 class="text-slate-900 font-bold text-base mb-4 flex items-center gap-2">
        <Globe class="w-4 h-4 text-cyan-500" />
        Active Proxy Routes
      </h2>
      <div class="overflow-x-auto">
        <table class="table table-sm w-full">
          <thead>
            <tr class="text-slate-400 border-slate-200/80">
              <th>Hostname / Prefix</th>
              <th>Upstream Target</th>
              <th>TLS Mode</th>
              <th>Tunnel</th>
              <th class="w-12 text-right">Delete</th>
            </tr>
          </thead>
          <tbody class="text-slate-700 text-sm">
            {#each routes as route}
              <tr class="border-slate-100 hover:bg-slate-50/50">
                <td class="font-mono text-xs font-semibold text-slate-800">{formatRouteKey(route)}</td>
                <td class="font-mono text-xs">{route.upstream}</td>
                <td>
                  <span class="badge badge-sm border-slate-200 font-semibold text-[10px]
                    {route.tls === 'Auto' ? 'bg-cyan-50/50 text-cyan-700 border-cyan-200/60' : 'text-slate-600 border-slate-200'}">
                    {route.tls}
                  </span>
                </td>
                <td>
                  {#if route.tunnel}
                    <span class="badge badge-sm text-violet-700 border-violet-200/60 font-semibold text-[10px]">{route.tunnel}</span>
                  {:else}
                    <span class="opacity-25">—</span>
                  {/if}
                </td>
                <td class="text-right">
                  <button class="btn btn-error btn-xs btn-outline rounded-md p-1 border-slate-100 hover:border-rose-300 hover:bg-rose-50 hover:text-rose-700" 
                    onclick={() => onDelete(formatRouteKey(route))}>
                    <Trash2 class="w-3.5 h-3.5" />
                  </button>
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="5" class="text-center py-12 text-slate-400 text-sm">No routes configured in database.</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  </div>

  <!-- Add Route Card -->
  <div class="card bg-white border border-slate-200/80 rounded-xl shadow-xs h-fit">
    <div class="card-body p-6">
      <h2 class="text-slate-900 font-bold text-base mb-4 flex items-center gap-2">
        <Plus class="w-4 h-4 text-cyan-500" />
        Add Proxy Route
      </h2>
      <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="space-y-4">
        <div class="form-control">
          <label class="label py-1" for="route-key-input">
            <span class="label-text text-slate-600 font-bold text-xs">Hostname / Path Prefix</span>
          </label>
          <input id="route-key-input" type="text" placeholder="e.g. app.localhost/api" class="input input-bordered w-full input-sm rounded-md focus:border-cyan-500 focus:outline-hidden" bind:value={newKey} required />
        </div>

        <div class="form-control">
          <label class="label py-1" for="route-upstream-input">
            <span class="label-text text-slate-600 font-bold text-xs">Upstream Address</span>
          </label>
          <input id="route-upstream-input" type="text" placeholder="e.g. 127.0.0.1:4000" class="input input-bordered w-full input-sm rounded-md focus:border-cyan-500 focus:outline-hidden" bind:value={newUpstream} required />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="form-control">
            <label class="label py-1" for="route-tls-select">
              <span class="label-text text-slate-600 font-bold text-xs">TLS Mode</span>
            </label>
            <select id="route-tls-select" class="select select-bordered select-sm w-full rounded-md focus:border-cyan-500" bind:value={newTls}>
              <option value="Auto">Auto (TLS)</option>
              <option value="Off">Off (HTTP)</option>
            </select>
          </div>

          <div class="form-control">
            <label class="label py-1" for="route-tunnel-input">
              <span class="label-text text-slate-600 font-bold text-xs">Tunnel Name</span>
            </label>
            <input id="route-tunnel-input" type="text" placeholder="Optional" class="input input-bordered w-full input-sm rounded-md focus:border-cyan-500 focus:outline-hidden" bind:value={newTunnel} />
          </div>
        </div>

        <button type="submit" class="btn btn-sm w-full mt-4 bg-cyan-500 hover:bg-cyan-600 border-none rounded-md font-bold text-white shadow-xs">
          Register Route
        </button>
      </form>
    </div>
  </div>
</div>
