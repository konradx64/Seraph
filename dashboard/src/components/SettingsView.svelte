<script>
  import { Sliders, Server, Database, Activity } from '@lucide/svelte';
</script>

<div class="card bg-white border border-slate-200/80 rounded-xl shadow-xs max-w-4xl animate-fade-in">
  <div class="card-body p-6">
    <h2 class="text-slate-900 font-bold text-base mb-6 flex items-center gap-2">
      <Sliders class="w-4 h-4 text-cyan-500" />
      System Configuration
    </h2>

    <div class="space-y-4 text-sm">
      <div class="flex justify-between items-center py-3 border-b border-slate-100">
        <div class="flex items-center gap-2">
          <Server class="w-4.5 h-4.5 text-slate-400" />
          <span class="font-semibold text-slate-700">HTTP Proxy Listener</span>
        </div>
        <span class="font-mono border border-slate-200/60 px-2 py-0.5 rounded text-xs text-slate-600">0.0.0.0:8080</span>
      </div>

      <div class="flex justify-between items-center py-3 border-b border-slate-100">
        <div class="flex items-center gap-2">
          <Server class="w-4.5 h-4.5 text-slate-400" />
          <span class="font-semibold text-slate-700">HTTPS Proxy Listener</span>
        </div>
        <span class="font-mono border border-slate-200/60 px-2 py-0.5 rounded text-xs text-slate-600">0.0.0.0:8443</span>
      </div>

      <div class="flex justify-between items-center py-3 border-b border-slate-100">
        <div class="flex items-center gap-2">
          <Server class="w-4.5 h-4.5 text-slate-400" />
          <span class="font-semibold text-slate-700">Admin Control Port</span>
        </div>
        <span class="font-mono border border-slate-200/60 px-2 py-0.5 rounded text-xs text-slate-600">127.0.0.1:9090</span>
      </div>

      <div class="flex justify-between items-center py-3 border-b border-slate-100">
        <div class="flex items-center gap-2">
          <Database class="w-4.5 h-4.5 text-slate-400" />
          <span class="font-semibold text-slate-700">SQLite Database Path</span>
        </div>
        <span class="font-mono border border-slate-200/60 px-2 py-0.5 rounded text-xs text-slate-600">seraph.db</span>
      </div>

      <div class="flex justify-between items-center py-3 border-b border-slate-100">
        <div class="flex items-center gap-2">
          <Activity class="w-4.5 h-4.5 text-slate-400" />
          <span class="font-semibold text-slate-700">Event Stream Engine</span>
        </div>
        <span class="font-semibold text-slate-600 text-xs flex items-center gap-1.5">
          <span class="w-1.5 h-1.5 rounded-full bg-cyan-500"></span>
          Tokio Broadcast Channel / SSE
        </span>
      </div>
    </div>
  </div>
</div>
