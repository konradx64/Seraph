<script>
  import logo from '../assets/logo.png';
  import { 
    Globe, 
    Key, 
    Terminal, 
    Sliders, 
    Wifi, 
    WifiOff,
    LayoutDashboard
  } from '@lucide/svelte';

  // Svelte 5 props definition
  let { status = 'Disconnected', activeTab = $bindable('routes') } = $props();
</script>

<header class="sticky top-0 z-50 bg-white/95 backdrop-blur border-b border-slate-100 px-6 py-3 flex items-center justify-between shadow-xs">
  <div class="flex items-center gap-3">
    <img src={logo} alt="Seraph Logo" class="w-8 h-8 object-contain" />
    <span class="text-lg font-black tracking-tight text-slate-900">Seraph</span>
  </div>

  <!-- Central Navigation Menu -->
  <nav class="flex gap-1 border border-slate-200/60 p-0.5 rounded-lg bg-white">
    <button class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-md text-xs font-bold transition-all 
      {activeTab === 'overview' ? 'bg-cyan-500 text-white shadow-xs' : 'text-slate-600 hover:text-slate-900'}" 
      onclick={() => activeTab = 'overview'}>
      <LayoutDashboard class="w-3.5 h-3.5" />
      Overview
    </button>
    <button class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-md text-xs font-bold transition-all 
      {activeTab === 'routes' ? 'bg-cyan-500 text-white shadow-xs' : 'text-slate-600 hover:text-slate-900'}" 
      onclick={() => activeTab = 'routes'}>
      <Globe class="w-3.5 h-3.5" />
      Routes
    </button>
    <button class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-md text-xs font-bold transition-all 
      {activeTab === 'certs' ? 'bg-cyan-500 text-white shadow-xs' : 'text-slate-600 hover:text-slate-900'}" 
      onclick={() => activeTab = 'certs'}>
      <Key class="w-3.5 h-3.5" />
      Certificates
    </button>
    <button class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-md text-xs font-bold transition-all 
      {activeTab === 'logs' ? 'bg-cyan-500 text-white shadow-xs' : 'text-slate-600 hover:text-slate-900'}" 
      onclick={() => activeTab = 'logs'}>
      <Terminal class="w-3.5 h-3.5" />
      Logs
    </button>
    <button class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-md text-xs font-bold transition-all 
      {activeTab === 'settings' ? 'bg-cyan-500 text-white shadow-xs' : 'text-slate-600 hover:text-slate-900'}" 
      onclick={() => activeTab = 'settings'}>
      <Sliders class="w-3.5 h-3.5" />
      Settings
    </button>
  </nav>

  <!-- Right Indicators -->
  <div class="flex items-center gap-3">
    <span class="text-[10px] font-bold px-2 py-0.5 rounded-md bg-white text-slate-500 border border-slate-200">v0.1.0</span>
    <div class="badge badge-sm gap-1.5 py-2.5 px-3.5 font-bold border-slate-200/60
      {status === 'Connected' ? 'bg-cyan-50/50 text-cyan-700 border-cyan-200/60' : 'bg-amber-50/50 text-amber-700 border-amber-200/60'}">
      {#if status === 'Connected'}
        <Wifi class="w-3 h-3 text-cyan-600 animate-pulse" />
      {:else}
        <WifiOff class="w-3 h-3 text-amber-600" />
      {/if}
      {status}
    </div>
  </div>
</header>
