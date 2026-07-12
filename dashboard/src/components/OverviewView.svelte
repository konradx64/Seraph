<script>
  import { 
    Globe, 
    Key, 
    Terminal, 
    Activity, 
    Wifi, 
    WifiOff, 
    Database, 
    Server,
    ArrowRight,
    Sliders
  } from '@lucide/svelte';

  // Svelte 5 props
  let { 
    routes = [], 
    certs = [], 
    logs = [], 
    status = 'Disconnected', 
    onSwitchTab 
  } = $props();
</script>

<div class="space-y-6 animate-fade-in">
  <!-- Stats Summary Grid -->
  <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
    <div class="stat bg-white border border-slate-200/80 rounded-xl">
      <div class="stat-title text-slate-500 font-semibold text-xs flex items-center gap-1.5 mb-1">
        <Server class="w-3.5 h-3.5 text-slate-400" />
        Gateway Status
      </div>
      <div class="stat-value text-slate-900 font-extrabold text-2xl flex items-center gap-2">
        <span class="w-2.5 h-2.5 rounded-full bg-current {status === 'Connected' ? 'text-emerald-500 animate-pulse' : 'text-amber-500'}"></span>
        {status}
      </div>
      <div class="stat-desc text-slate-400 mt-1 font-medium">Control port: 127.0.0.1:9090</div>
    </div>

    <div class="stat bg-white border border-slate-200/80 rounded-xl">
      <div class="stat-title text-slate-500 font-semibold text-xs flex items-center gap-1.5 mb-1">
        <Globe class="w-3.5 h-3.5 text-slate-400" />
        Proxy Routes
      </div>
      <div class="stat-value text-slate-900 font-extrabold text-2xl">
        {routes.length} <span class="text-xs font-semibold text-slate-400">configured</span>
      </div>
      <div class="stat-desc text-slate-400 mt-1 font-medium">Traffic listener: port 8080</div>
    </div>

    <div class="stat bg-white border border-slate-200/80 rounded-xl">
      <div class="stat-title text-slate-500 font-semibold text-xs flex items-center gap-1.5 mb-1">
        <Key class="w-3.5 h-3.5 text-slate-400" />
        Certificates
      </div>
      <div class="stat-value text-slate-900 font-extrabold text-2xl">
        {certs.length} <span class="text-xs font-semibold text-slate-400">registered</span>
      </div>
      <div class="stat-desc text-slate-400 mt-1 font-medium">TLS handshakes: port 8443</div>
    </div>
  </div>

  <!-- Primary Sections -->
  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
    <!-- Recent Activity log -->
    <div class="card lg:col-span-2 bg-white border border-slate-200/80 rounded-xl">
      <div class="card-body p-6">
        <h2 class="text-slate-900 font-bold text-base mb-4 flex items-center gap-2">
          <Activity class="w-4 h-4 text-cyan-500" />
          Recent Proxy Activity
        </h2>
        
        <div class="space-y-3">
          {#each logs.slice(0, 6) as log}
            <div class="flex items-start gap-3 p-3 border border-slate-100 rounded-lg bg-slate-50/20 hover:bg-slate-50/50 transition-colors text-sm">
              <span class="text-slate-400 font-mono text-xs mt-0.5">[{log.time}]</span>
              <span class="text-slate-700 font-semibold text-xs">{log.text}</span>
            </div>
          {:else}
            <div class="flex flex-col items-center justify-center py-16 text-slate-400 gap-2 border border-dashed border-slate-200 rounded-lg">
              <Terminal class="w-8 h-8 opacity-40 text-slate-500" />
              <span class="text-xs font-semibold">Waiting for live gateway requests...</span>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <!-- Quick Actions Menu -->
    <div class="card bg-white border border-slate-200/80 rounded-xl h-fit">
      <div class="card-body p-6">
        <h2 class="text-slate-900 font-bold text-base mb-4 flex items-center gap-2">
          <Sliders class="w-4 h-4 text-cyan-500" />
          Quick Actions
        </h2>

        <div class="flex flex-col gap-3">
          <button class="flex items-center justify-between p-4 border border-slate-200/80 rounded-xl hover:border-cyan-400/80 hover:bg-cyan-50/30 transition-all text-left group" 
            onclick={() => onSwitchTab('routes')}>
            <div>
              <div class="font-bold text-xs text-slate-800">Manage Proxy Routes</div>
              <div class="text-[10px] text-slate-500 mt-0.5">Add, modify or delete reverse proxy targets</div>
            </div>
            <ArrowRight class="w-4 h-4 text-slate-400 group-hover:text-cyan-500 transition-colors" />
          </button>

          <button class="flex items-center justify-between p-4 border border-slate-200/80 rounded-xl hover:border-cyan-400/80 hover:bg-cyan-50/30 transition-all text-left group" 
            onclick={() => onSwitchTab('certs')}>
            <div>
              <div class="font-bold text-xs text-slate-800">SSL Certificates</div>
              <div class="text-[10px] text-slate-500 mt-0.5">Register domain keys for secure TLS routing</div>
            </div>
            <ArrowRight class="w-4 h-4 text-slate-400 group-hover:text-cyan-500 transition-colors" />
          </button>

          <button class="flex items-center justify-between p-4 border border-slate-200/80 rounded-xl hover:border-cyan-400/80 hover:bg-cyan-50/30 transition-all text-left group" 
            onclick={() => onSwitchTab('logs')}>
            <div>
              <div class="font-bold text-xs text-slate-800">Live Traffic Stream</div>
              <div class="text-[10px] text-slate-500 mt-0.5">Open fullscreen developer request stream console</div>
            </div>
            <ArrowRight class="w-4 h-4 text-slate-400 group-hover:text-cyan-500 transition-colors" />
          </button>
        </div>
      </div>
    </div>
  </div>
</div>
