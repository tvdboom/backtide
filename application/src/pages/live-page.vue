<template>
  <div class="page">
    <section class="page-intro live-intro">
      <div><h2>Live session</h2><p>Apply a saved strategy to live bars with simulated fills.</p></div>
      <div v-if="sessionVisible" class="session-actions">
        <span
          class="status-pill"
          :class="{ running: activeSession, failed: sessionFailed }"
          :tabindex="sessionError ? 0 : undefined"
          :aria-describedby="sessionError ? 'session-error-tooltip' : undefined"
        >
          <span class="status-dot"/> {{ sessionStatusLabel }}
          <span v-if="sessionError" id="session-error-tooltip" class="session-status-tooltip" role="tooltip">{{ sessionError }}</span>
        </span>
        <button v-if="state.status === 'paused'" class="secondary" @click="resume"><Play :size="15"/> Resume</button>
        <button v-else-if="activeSession" class="secondary" @click="pause"><Pause :size="15"/> Pause</button>
        <span v-if="activeSession && hasStrategy" class="action-help"><button class="secondary" @click="cancelAll"><ListX :size="15" aria-hidden="true" /> Cancel orders</button><span class="action-popover" role="tooltip">Cancel every open simulated order before the next market update.</span></span>
        <span v-if="activeSession && hasStrategy" class="action-help"><button class="secondary" @click="flatten"><FoldVertical :size="15" aria-hidden="true" /> Flatten</button><span class="action-popover" role="tooltip">Close all simulated positions on the next market update.</span></span>
        <button v-if="activeSession" class="danger secondary" @click="stop"><Square :size="15"/> Stop</button>
        <template v-else>
          <button class="primary" @click="newConfiguration"><Plus :size="16"/> New configuration</button>
          <button class="secondary" @click="openSessionHistory"><History :size="16"/> Session history</button>
        </template>
      </div>
    </section>

    <section v-if="state.config?.mode === 'replay'" class="panel replay-playback-panel">
      <div><span class="eyebrow">Recorded playback</span><h3>{{ replaySpeedLabel }}</h3><p>{{ replayWarmupLabel }}</p></div>
      <div class="replay-progress"><div class="replay-progress-track" role="progressbar" aria-label="Replay progress" :aria-valuemin="0" :aria-valuemax="replayStatus.total_events || 0" :aria-valuenow="replayStatus.processed_events || 0"><span :style="{ width: `${replayProgress}%` }"/></div><small>{{ replayStatus.processed_events || 0 }} / {{ replayStatus.total_events || 0 }} events · {{ replayProgress.toFixed(1) }}% · {{ replayDurationLabel }}</small></div>
    </section>

    <section v-if="!sessionVisible" class="live-setup">
      <form class="panel experiment-builder live-builder" @submit.prevent="start">
        <div class="tabs live-form-tabs" role="tablist" aria-label="Live session setup steps">
          <button v-for="(item, index) in setupTabs" :key="item" type="button" :class="{ active: setupTab === index }" @click="setupTab = index"><span>{{ index + 1 }}</span>{{ item }}</button>
        </div>

        <div v-if="setupTab === 0" class="form-section">
          <div class="section-copy"><h3>Market data</h3><p>Choose the live feed, instruments, and candle interval for this session.</p></div>
          <div class="form-grid two">
            <fieldset class="provider-field wide">
              <legend>Provider<FieldInfo text="Select the exchange WebSocket that supplies live market data." /></legend>
              <div class="provider-logo-select" role="radiogroup" aria-label="Live market data provider">
                <button
                  v-for="provider in providers"
                  :key="provider"
                  type="button"
                  role="radio"
                  :aria-checked="form.provider === provider"
                  :aria-label="`${title(provider)}${providerAvailable(provider) ? '' : ', unavailable'}`"
                  :class="{ selected: form.provider === provider, unavailable: !providerAvailable(provider) }"
                  :disabled="!providerAvailable(provider)"
                  @click="selectProvider(provider)"
                >
                  <img :src="providerLogos[provider]" alt="" />
                  <span v-if="!providerAvailable(provider)">Unavailable</span>
                </button>
              </div>
            </fieldset>
            <div class="field-label wide symbol-select-field"><span>Symbols</span><FieldInfo text="Choose the instruments to subscribe to and monitor during this session." /><SearchSelect v-model="form.symbols" :options="liveSymbols" :descriptions="liveSymbolNames" :logos="liveSymbolLogos" :option-details="liveSymbolDetails" :loading="loadingLiveSymbols" clearable clear-label="symbols" allow-custom input-id="live-symbols" label="Live symbols" placeholder="Search the provider catalog, e.g. BTC-USD…" /><small v-if="liveSymbolError" class="negative">{{ liveSymbolError }}</small></div>
            <div class="field-label interval-picker-field live-interval-field wide">
              <span>Interval</span>
              <FieldInfo text="Set the candle duration used for strategy evaluation and monitoring." />
              <IntervalPicker v-model="form.interval" :options="intervals" :disabled-options="disabledProviderIntervals" input-id="live-interval" label="Live session interval" />
            </div>
          </div>
        </div>

        <div v-if="setupTab === 1" class="form-section">
          <div class="section-copy"><h3>Starting portfolio</h3><p>Set the simulated cash balance and reporting currency.</p></div>
          <div class="portfolio-basics live-portfolio-basics">
            <label class="live-cash-field">Initial cash<FieldInfo text="Set the simulated cash balance available when the live session starts." /><input id="live-initial-cash" v-model.number="form.config.initial_cash" type="number" min="0" step="100" /></label>
            <div class="field-label currency-picker-field live-base-currency">
              <span>Base currency</span>
              <FieldInfo text="Choose the currency used to value the simulated account and report profit and loss." />
              <CurrencySelect
                :model-value="form.config.base_currency"
                :options="bootstrap.enums.currencies"
                input-id="live-base-currency"
                @update:model-value="setBaseCurrency"
              />
            </div>
          </div>
        </div>

        <div v-if="setupTab === 2" class="form-section">
          <div class="section-copy"><h3>Trading logic</h3><p>Select saved strategies and optional indicators for the live feed.</p></div>
          <div class="form-grid two">
            <div class="field-label wide"><span>Strategies</span><FieldInfo text="Select saved strategies to evaluate; leave this empty for market monitoring only." /><SearchSelect v-model="form.strategies" :options="strategyOptions" :option-icons="strategyOptionIcons" clearable clear-label="strategies" input-id="live-strategies" label="Live strategies" placeholder="Select one or more saved strategies..."/><small>Leave empty to monitor the feed without orders.</small></div>
            <div class="field-label wide"><span>Indicators</span><FieldInfo text="Add optional indicators to calculate during the session." /><SearchSelect v-model="form.indicators" :options="indicatorOptions" :option-icons="indicatorOptionIcons" clearable clear-label="indicators" input-id="live-indicators" label="Live indicators" placeholder="Select optional indicators..."/><small>Strategy-required indicators are added automatically.</small></div>
          </div>
        </div>

        <div v-if="setupTab === 3" class="form-section">
          <div class="section-copy"><h3>Performance metrics</h3><p>Choose the measures updated while the live session runs, then drag them into the order used by live monitoring.</p></div>
          <div class="form-grid two">
            <div class="field-label wide"><span>Metrics</span><FieldInfo text="Choose the performance measures updated while the session runs." /><SearchSelect v-model="form.config.metrics" :options="liveMetricOptions" :descriptions="liveMetricDescriptions" option-name-first reorderable input-id="live-metrics" label="Live metrics" placeholder="Select live-compatible metrics..."/><button type="button" class="text-button metric-clear-button" aria-label="Clear all live metrics" :disabled="form.config.metrics.length === 0" @click="clearLiveMetrics"><X :size="14" /> Clear all metrics</button></div>
            <TransitionGroup v-if="selectedLiveMetrics.length" tag="section" name="metric-reorder" class="selection-insights metric-selection-list wide" aria-label="Selected live metric details">
              <article v-for="(item, index) in selectedLiveMetrics" :key="item.key" class="asset-selection-card compact-card metric-selection-card" :class="{ dragging: draggedMetricKey === item.key, 'drop-target': dragOverMetricKey === item.key }" draggable="true" @dragstart="startLiveMetricDrag($event, item.key)" @dragover.prevent="dragOverLiveMetric($event, item.key)" @drop.prevent="finishLiveMetricDrag" @dragend="finishLiveMetricDrag">
                <header>
                  <span class="metric-icon"><LibraryAssetIcon kind="metric" :builtin="item.builtin" :size="18" /></span>
                  <span><strong>{{ item.name }}</strong><small>{{ item.builtin ? 'Built-in' : 'Custom' }}</small></span>
                  <span class="metric-order-actions">
                    <span class="metric-drag-handle" :title="`Drag ${item.name} to reorder`"><GripVertical :size="16" aria-hidden="true" /></span>
                    <span class="metric-order-number" :aria-label="`Live metric position ${index + 1}`">{{ index + 1 }}</span>
                    <button type="button" class="icon-button" :aria-label="`Move ${item.name} up`" :disabled="index === 0" @click="moveLiveMetric(item.key, -1)"><ChevronUp :size="15" /></button>
                    <button type="button" class="icon-button" :aria-label="`Move ${item.name} down`" :disabled="index === selectedLiveMetrics.length - 1" @click="moveLiveMetric(item.key, 1)"><ChevronDown :size="15" /></button>
                    <button type="button" class="icon-button metric-remove-button" :aria-label="`Remove ${item.name} live metric`" @click="removeLiveMetric(item.key)"><X :size="15" /></button>
                  </span>
                </header>
                <p>{{ item.description }}</p>
              </article>
            </TransitionGroup>
          </div>
        </div>

        <div v-if="setupTab === 4" class="form-section">
          <div class="section-copy"><h3>Execution model</h3><p>Model commissions, slippage, fills, and supported order types.</p></div>
          <div class="settings-stack">
            <fieldset class="settings-group">
              <legend>Fees and price impact</legend>
              <div class="form-grid three">
                <label>Commission (%)<FieldInfo text="Apply this percentage fee to the value of every simulated fill." /><input v-model.number="form.config.commission_pct" type="number" min="0" step="0.01" /></label>
                <label>Fixed commission<FieldInfo text="Apply this fixed cash fee to every simulated fill." /><input v-model.number="form.config.commission_fixed" type="number" min="0" step="0.01" /></label>
                <label>Slippage (%)<FieldInfo text="Move simulated fill prices against the order by this percentage." /><input v-model.number="form.config.slippage" type="number" min="0" step="0.01" /></label>
              </div>
            </fieldset>
            <fieldset class="settings-group">
              <legend>Order handling</legend>
              <div class="form-grid two">
                <ToggleField v-model="form.config.partial_fills" label="Volume-constrained fills" description="Limit fills to available candle volume." help="Restrict simulated fills to a share of the volume reported by each candle." />
                <label v-show="form.config.partial_fills">Max volume participation (%)<FieldInfo text="Set the largest percentage of a candle's volume that one simulated fill may consume." /><input v-model.number="form.config.max_volume_participation" type="number" min="0.01" max="100" step="0.01"/></label>
                <div class="field-label wide"><span>Allowed order types</span><FieldInfo text="Choose which simulated order instructions strategies may submit." /><SearchSelect v-model="form.config.allowed_order_types" :options="orderTypes" :descriptions="orderTypeDescriptions" plain-options input-id="live-order-types" label="Allowed order types"/></div>
              </div>
            </fieldset>
          </div>
        </div>

        <div v-if="setupTab === 5" class="form-section">
          <div class="section-copy"><h3>Risk controls</h3><p>Bound leverage, short exposure, concentration, and account drawdown.</p></div>
          <div class="settings-stack">
            <fieldset class="settings-group">
              <legend>Exposure</legend>
              <div class="form-grid two">
                <ToggleField v-model="form.config.allow_short" label="Short selling" description="Allow selling assets not currently held." help="Allow strategies to sell assets they do not currently hold." />
                <label>Max position (%)<FieldInfo text="Cap a single position at this percentage of simulated account equity." /><input v-model.number="form.config.max_position_size" type="number" min="0.01" max="100" step="0.01"/></label>
                <label>Maximum drawdown halt (%)<FieldInfo text="Stop opening new positions when account drawdown reaches this percentage; zero disables the halt." /><input v-model.number="form.config.max_drawdown" type="number" min="0" max="100" step="1"/><small>Zero disables the kill switch.</small></label>
              </div>
            </fieldset>
            <fieldset class="settings-group">
              <legend>Margin</legend>
              <div class="form-grid two">
                <ToggleField v-model="form.config.allow_margin" label="Margin trading" description="Allow positions to use borrowed funds." help="Allow the simulated account to borrow funds up to the leverage limit." />
                <template v-if="form.config.allow_margin">
                  <label>Maximum leverage<FieldInfo text="Limit total simulated exposure to this multiple of account equity." /><input v-model.number="form.config.max_leverage" type="number" min="1" step="0.1"/></label>
                  <label>Initial margin (%)<FieldInfo text="Require this percentage of a new leveraged position as opening collateral." /><input v-model.number="form.config.initial_margin" type="number" min="0" max="100" step="1"/></label>
                  <label>Maintenance margin (%)<FieldInfo text="Require this collateral percentage to keep a leveraged position open." /><input v-model.number="form.config.maintenance_margin" type="number" min="0" max="100" step="1"/></label>
                  <label>Margin interest (% annual)<FieldInfo text="Charge this annual rate on simulated borrowed cash." /><input v-model.number="form.config.margin_interest" type="number" min="0" step="0.1"/></label>
                  <label>Short borrow (% annual)<FieldInfo text="Charge this annual rate on the value of simulated short positions." /><input v-model.number="form.config.borrow_rate" type="number" min="0" step="0.1"/></label>
                </template>
              </div>
            </fieldset>
          </div>
        </div>

        <div v-if="setupTab === 6" class="form-section">
          <div class="section-copy"><h3>Engine behavior</h3><p>Choose warm-up, metric assumptions, retention, and candle timing for the live session.</p></div>
          <fieldset class="settings-group">
            <legend>Session processing</legend>
            <div class="form-grid two">
              <label>Warm-up bars<FieldInfo text="Load this many historical bars before live evaluation so strategies and indicators have enough context." /><input v-model.number="form.warmup_bars" type="number" min="0" max="100000" step="100"/><small>Seeds strategy and indicator history without placing orders.</small></label>
              <label>Risk-free rate (%)<FieldInfo text="Set the annual reference return used by risk-adjusted metrics such as Sharpe ratio." /><input v-model.number="form.config.risk_free_rate" type="number" step="0.01"/></label>
              <label>History limit<FieldInfo text="Limit how many recent live bars and events are retained in memory." /><input v-model.number="form.config.max_history" type="number" min="100" max="100000" step="100" /></label>
              <ToggleField v-model="form.config.trade_on_partial" label="Trade partial bars" description="Evaluate strategies before candles close." help="Evaluate strategies on in-progress candles instead of waiting for each candle to close." />
            </div>
          </fieldset>
        </div>

        <div class="form-footer"><button v-if="setupTab" type="button" class="secondary" @click="setupTab--"><ChevronLeft :size="16"/> Back</button><span class="form-spacer"/><button v-if="setupTab < setupTabs.length - 1" type="button" class="secondary" @click="setupTab++">Continue <ChevronRight :size="16"/></button><button type="submit" class="primary live-button" :disabled="starting || !available(form.provider) || !form.symbols.length"><span v-if="starting" class="spinner small"/><Radio v-else :size="16"/> {{ starting ? 'Connecting…' : 'Start live session' }}</button></div>
      </form>
      <aside class="panel safety-panel"><ShieldCheck :size="28"/><h3>Simulation only</h3><p>Backtide calculates hypothetical fills locally. It does not connect to a brokerage account or submit real orders.</p><ul><li>Real-time provider WebSockets</li><li>Simulated commission and slippage</li><li>Local-only portfolio state</li><li>Bounded event history</li></ul></aside>
    </section>

    <template v-else>
      <section v-if="strategyNames.length" class="panel live-strategy-panel">
        <div class="panel-header"><div><span class="eyebrow">Strategy</span><h3>Live strategy view</h3></div><small>Metrics, portfolio, orders, and risk update for the selected strategy.</small></div>
        <div class="strategy-switcher live-strategy-switcher" role="tablist" aria-label="Live strategies">
          <button v-for="name in strategyNames" :key="name" type="button" role="tab" :aria-selected="activeStrategyName === name" :class="{ active: activeStrategyName === name }" @click="selectedStrategyName = name"><Bot :size="15" aria-hidden="true"/><span>{{ name }}</span></button>
        </div>
      </section>
      <section v-if="hasStrategy" class="metric-grid live-metrics">
        <article class="metric-card"><span>Equity</span><strong>{{ money(activeStrategySnapshot.equity) }}</strong></article>
        <article class="metric-card"><span>Realized P&amp;L</span><strong :class="tone(activeStrategySnapshot.realized_pnl)">{{ money(activeStrategySnapshot.realized_pnl) }}</strong></article>
        <article class="metric-card"><span>Unrealized P&amp;L</span><strong :class="tone(activeStrategySnapshot.unrealized_pnl)">{{ money(activeStrategySnapshot.unrealized_pnl) }}</strong></article>
        <article class="metric-card"><span>Open positions</span><strong>{{ Object.keys(activeStrategySnapshot.portfolio?.positions || {}).length }}</strong></article>
      </section>
      <section class="live-dashboard" :class="{ 'has-portfolio': hasStrategy }">
        <article v-if="hasStrategy" class="panel table-panel live-portfolio-panel"><div class="panel-header"><div><span class="eyebrow">Portfolio</span><h3>Positions &amp; cash</h3></div></div><div class="data-table-wrap"><table class="data-table"><thead><tr><th>Asset</th><th class="number">Amount</th></tr></thead><tbody><tr v-for="(quantity, symbol) in activeStrategySnapshot.portfolio?.positions" :key="symbol"><td><span class="live-asset-cell"><img v-if="symbolLogo(symbol)" :src="symbolLogo(symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ symbol.slice(0, 1) }}</span><strong>{{ symbol }}</strong></span></td><td class="number">{{ positionAmount(quantity) }}</td></tr><tr v-for="(amount, currency) in activeStrategySnapshot.portfolio?.cash" :key="currency"><td>{{ currency }} cash</td><td class="number">{{ money(amount, currency) }}</td></tr></tbody></table></div></article>
        <article class="panel live-chart">
          <div class="panel-header"><div><span class="eyebrow">{{ liveChartEyebrow }}</span><h3>{{ liveChartTitle }}</h3></div></div>
          <ChartPanel :figure="liveFigure" :empty-message="liveChartMessage" />
          <div v-if="hasStrategy" class="chart-tabs live-plot-tabs" role="tablist" aria-label="Live chart views">
            <button v-for="item in livePlotTabs" :key="item.id" type="button" role="tab" :aria-selected="livePlot === item.id" :class="{ active: livePlot === item.id }" @click="livePlot = item.id"><component :is="item.icon" :size="16" aria-hidden="true"/><span>{{ item.label }}</span></button>
          </div>
        </article>
        <article class="panel quote-board">
          <div class="panel-header"><div><span class="eyebrow">Latest prices</span><h3>Watchlist</h3></div></div>
          <div v-for="item in watchlist" :key="item.symbol" class="quote-row">
            <img v-if="symbolLogo(item.symbol)" :src="symbolLogo(item.symbol)" class="asset-avatar" alt="" @error="markSymbolLogoFailed(item.symbol)" />
            <span v-else class="asset-avatar" aria-hidden="true">{{ item.symbol.slice(0, 1) }}</span>
            <span><strong>{{ item.symbol }}</strong></span>
            <strong v-if="item.price !== undefined">{{ price(item.price, item.symbol) }}</strong>
            <small v-else class="quote-waiting">Waiting for price</small>
          </div>
          <div v-if="!watchlist.length" class="empty-state compact"><p>Waiting for the first market update…</p></div>
        </article>
      </section>
      <section v-if="hasStrategy" class="live-tables">
        <article class="panel table-panel execution-panel">
          <div class="panel-header"><div><span class="eyebrow">Execution</span><h3>Recent orders</h3></div><small>Up to 12 latest</small></div>
          <div class="data-table-wrap live-execution-table">
            <table class="data-table">
              <thead><tr><th>Time</th><th>Symbol</th><th>Side</th><th>Type</th><th class="number">Quantity</th><th class="number">Fill price</th><th class="number">P&amp;L</th><th class="number">Commission</th><th>Status</th></tr></thead>
              <tbody>
                <tr v-for="(fill, index) in fills" :key="fillKey(fill, index)">
                  <td><time class="execution-time">{{ fillTime(fill) }}</time></td>
                  <td><span class="execution-symbol"><span class="live-asset-cell"><img v-if="symbolLogo(fill.order?.symbol)" :src="symbolLogo(fill.order?.symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(fill.order?.symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ String(fill.order?.symbol || '?').slice(0, 1) }}</span><strong>{{ fill.order?.symbol || '—' }}</strong></span></span></td>
                  <td class="order-side" :class="fillSideClass(fill)">{{ fillSide(fill) }}</td>
                  <td>{{ fillOrderType(fill) }}</td>
                  <td class="number">{{ fillQuantity(fill) }}</td>
                  <td class="number">{{ fill.fill_price == null ? '—' : price(fill.fill_price, fill.order?.symbol) }}</td>
                  <td class="number" :class="tone(realizedTradePnl(fill))">{{ realizedTradePnl(fill) == null ? '—' : money(realizedTradePnl(fill)) }}</td>
                  <td class="number">{{ fill.commission == null ? '—' : money(fill.commission) }}</td>
                  <td><ExecutionStatus :status="fill.status" :reason="fill.reason" /></td>
                </tr>
              </tbody>
            </table>
            <div v-if="!fills.length" class="empty-state compact"><p>{{ executionEmptyMessage }}</p></div>
          </div>
        </article>
      </section>
      <section v-if="hasStrategy" class="split-grid live-observability">
        <article class="panel">
          <div class="panel-header"><div><span class="eyebrow">Session telemetry</span><h3>Trading, exposure &amp; controls</h3></div><span v-if="activeStrategySnapshot.trading_halted" class="badge error">HALTED</span></div>
          <dl class="config-summary">
            <div v-for="item in riskMetrics" :key="item.key" class="risk-metric" :data-risk-metric="item.key"><FieldInfo class="risk-metric-help" :text="item.help"/><dt><component :is="item.icon" :size="15" aria-hidden="true"/><span>{{ item.label }}</span></dt><dd>{{ item.value }}</dd></div>
            <div v-for="item in latestIndicators" :key="item.key" class="live-indicator"><dt>{{ item.name }} · {{ item.symbol }}</dt><dd>{{ item.value }}</dd></div>
          </dl>
          <p v-if="activeStrategySnapshot.halt_reason" class="negative">{{ activeStrategySnapshot.halt_reason }}</p>
        </article>
        <article class="panel table-panel live-metrics-panel">
          <div class="panel-header"><div><span class="eyebrow">Live monitoring</span><h3>Metrics</h3></div></div>
          <section v-if="liveMetrics.length" class="data-table-wrap live-selected-metrics" aria-label="Selected live metrics">
            <table class="data-table live-metrics-table">
              <thead><tr><th>Metric</th><th class="number">Value</th></tr></thead>
              <tbody><tr v-for="item in liveMetrics" :key="item.key"><td>{{ item.label }}</td><td class="number" :class="tone(item.value)">{{ metricValue(item.metric, item.value) }}</td></tr></tbody>
            </table>
          </section>
          <p v-else class="live-metrics-empty">No metrics selected.</p>
        </article>
      </section>
      <article class="panel event-feed">
        <div class="panel-header"><div><span class="eyebrow">Live diagnostics</span><h3>Market event feed</h3></div><span>{{ updates.length }} buffered · {{ state.health?.received_events || 0 }} received · {{ state.health?.warmup_bars_loaded || 0 }} warmed</span></div>
        <div class="event-log" role="table" aria-label="Latest live market events">
          <div class="event-log-header" role="row">
            <span role="columnheader">Time</span>
            <span role="columnheader">Status</span>
            <span role="columnheader">Symbol</span>
            <span class="event-open" role="columnheader">Open</span>
            <span class="event-high" role="columnheader">High</span>
            <span class="event-low" role="columnheader">Low</span>
            <span class="event-close" role="columnheader">Close</span>
            <span class="event-volume" role="columnheader">Volume</span>
            <span class="event-fills" role="columnheader">Fills</span>
          </div>
          <div v-for="(update, index) in [...updates].reverse().slice(0, 50)" :key="index" class="event-log-row" role="row">
            <time role="cell">{{ eventTime(update) }}</time>
            <span class="badge" :class="update.market?.is_final ? 'success' : 'neutral'" role="cell">{{ update.market?.is_final ? 'CLOSED' : 'PARTIAL' }}</span>
            <span class="event-symbol" role="cell">
              <img v-if="symbolLogo(update.market?.symbol)" :src="symbolLogo(update.market?.symbol)" class="event-symbol-logo" alt="" @error="markSymbolLogoFailed(update.market?.symbol)" />
              <span v-else class="event-symbol-logo" aria-hidden="true">{{ String(update.market?.symbol || '?').slice(0, 1) }}</span>
              <strong>{{ update.market?.symbol || '—' }}</strong>
            </span>
            <span class="event-open" role="cell">{{ update.market?.open == null ? '—' : price(update.market.open, update.market.symbol) }}</span>
            <span class="event-high" role="cell">{{ update.market?.high == null ? '—' : price(update.market.high, update.market.symbol) }}</span>
            <span class="event-low" role="cell">{{ update.market?.low == null ? '—' : price(update.market.low, update.market.symbol) }}</span>
            <span class="event-close" role="cell">{{ update.market?.close == null ? '—' : price(update.market.close, update.market.symbol) }}</span>
            <span class="event-volume" role="cell">{{ update.market?.volume ?? '—' }}</span>
            <span class="event-fills" :class="{ positive: update.fills?.length }" role="cell">
              <template v-if="update.fills?.length">{{ update.fills.length }} fill{{ update.fills.length === 1 ? '' : 's' }}</template>
              <template v-else>—</template>
            </span>
          </div>
        </div>
      </article>
    </template>
  </div>
</template>

<script setup>
import {
  ArrowRightLeft,
  Bot,
  Braces,
  ChartLine,
  ChartNoAxesCombined,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  CircleDollarSign,
  FoldVertical,
  Gauge,
  GripVertical,
  History,
  ListX,
  Pause,
  Play,
  Plus,
  Radio,
  ReceiptText,
  Scale,
  Shapes,
  ShieldCheck,
  Square,
  SquareCode,
  TrendingDown,
  WalletCards,
  X
} from 'lucide-vue-next'
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, reactive, ref } from 'vue'
import { api, post, query } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import CurrencySelect from '../components/currency-select.vue'
import ExecutionStatus from '../components/execution-status.vue'
import FieldInfo from '../components/field-info.vue'
import IntervalPicker from '../components/interval-picker.vue'
import LibraryAssetIcon from '../components/library-asset-icon.vue'
import SearchSelect from '../components/search-select.vue'
import ToggleField from '../components/toggle-field.vue'
import { configuredCurrencyDecimals, configuredPlotlyDateTimeFormat, flattenFills, formatConfiguredCurrency, formatConfiguredTimeWithSeconds, instrumentLogoUrl, sessionEquitySeries } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast', 'live-status', 'navigate'])
const providers = ['kraken', 'binance', 'coinbase']
const currencyMetrics = new Set([
  'pnl',
  'final_equity',
  'expectancy',
  'avg_win',
  'avg_loss',
  'best_trade',
  'worst_trade'
])
const intervals = props.bootstrap.enums.intervals
const state = reactive({ status: 'idle', config: {}, snapshot: {}, updates: [], health: {}, error: null })
const portfolioDefaults = props.bootstrap.defaults?.portfolio || {}
const setupTabs = ['Market data', 'Portfolio', 'Strategy', 'Metrics', 'Execution', 'Risk', 'Engine']
const setupTab = ref(0)
const livePlot = ref('price')
const livePlotTabs = [
  { id: 'price', label: 'Price', icon: ChartLine, eyebrow: 'WebSocket market data', title: 'Live market prices' },
  { id: 'pnl', label: 'P&L', icon: CircleDollarSign, eyebrow: 'Strategy performance', title: 'Net simulated P&L' },
  { id: 'equity', label: 'Equity', icon: WalletCards, eyebrow: 'Account value', title: 'Simulated equity' },
  { id: 'exposure', label: 'Exposure', icon: ChartNoAxesCombined, eyebrow: 'Risk profile', title: 'Market exposure' },
  { id: 'drawdown', label: 'Drawdown', icon: TrendingDown, eyebrow: 'Risk profile', title: 'Drawdown from peak' }
]
const form = reactive(defaultLiveForm())
applyExperimentDraft()
const strategyOptions = computed(() => (props.bootstrap.strategies?.saved || []).map(item => item.name))
const indicatorOptions = computed(() => (props.bootstrap.indicators?.saved || []).map(item => item.name))
const strategyOptionIcons = computed(() => Object.fromEntries(
  (props.bootstrap.strategies?.saved || [])
    .map(item => [item.name, item.builtin ? Bot : SquareCode])))
const indicatorOptionIcons = computed(() => Object.fromEntries(
  (props.bootstrap.indicators?.saved || [])
    .map(item => [item.name, item.builtin ? Shapes : Braces])))
const liveMetricCatalog = computed(() => [
  ...(props.bootstrap.metrics?.builtin || []).filter(item =>
    !['alpha', 'excess_return'].includes(item.key)),
  ...(props.bootstrap.metrics?.saved || [])
])
const liveMetricOptions = computed(() => liveMetricCatalog.value.map(item => item.key))
const liveMetricDescriptions = computed(() => Object.fromEntries(liveMetricCatalog.value.map(item => [item.key, item.description])))
const selectedLiveMetrics = computed(() => form.config.metrics
  .map(key => liveMetricCatalog.value.find(item => item.key === key))
  .filter(Boolean))
const orderTypes = props.bootstrap.enums.order_types || form.config.allowed_order_types
const orderTypeDescriptions = computed(() => Object.fromEntries(orderTypes.map(item => [
  item,
  {
    market: 'Execute at the best available market price.',
    limit: 'Execute only at the chosen price or better.',
    stoploss: 'Trigger a protective market order at the stop price.',
    takeprofit: 'Close a position at the chosen profit target.',
    stoplimit: 'Trigger a limit order when the stop price is reached.',
    stoplosslimit: 'Trigger a protective limit order when the stop price is reached.',
    takeprofitlimit: 'Trigger a closing limit order when the profit target is reached.',
    trailingstop: 'Follow favorable prices and trigger a market order after a reversal.',
    trailingstoplimit: 'Follow favorable prices and trigger a limit order after a reversal.',
    settleposition: 'Close or settle an existing position without opening a new one.',
    cancel: 'Cancel an existing pending order.'
  }[String(item).replaceAll(/[^a-z]/gi, '').toLowerCase()] || 'Control how and when an order may be filled.'
])))
const starting = ref(false)
const draggedMetricKey = ref('')
const dragOverMetricKey = ref('')
let metricDragPreview = null
const dismissedSessionId = ref(null)
const failedSymbolLogos = ref(new Set())
const liveInstruments = ref([])
const loadingLiveSymbols = ref(false)
const liveSymbolError = ref('')
const liveSymbols = computed(() => liveInstruments.value.map(item => item.symbol))
const liveSymbolNames = computed(() => Object.fromEntries(liveInstruments.value.map(item => [
  item.symbol,
  item.name || `${title(form.provider)} spot market`
])))
const liveSymbolDetails = computed(() => Object.fromEntries(liveInstruments.value.map(item => [
  item.symbol,
  item
])))
const providerLogos = {
  binance: '/providers/binance.png',
  coinbase: '/providers/coinbase.png',
  kraken: '/providers/kraken.png'
}
const liveSymbolLogos = computed(() => Object.fromEntries([
  ...liveSymbols.value,
  ...form.symbols
].map(symbol => [
  symbol,
  instrumentLogoUrl(symbol, 'Crypto', props.bootstrap.display.logokit_api_key)
])))
let timer
let liveCatalogRequest = 0
const snapshot = computed(() => state.snapshot || {})
const activeSession = computed(() => ['running', 'paused'].includes(state.status))
const sessionFailed = computed(() => state.status === 'error')
const sessionError = computed(() => sessionFailed.value && state.error ? String(state.error) : '')
const currentSessionId = computed(() => state.id ?? '__session_without_id__')
const sessionVisible = computed(() =>
  state.status !== 'idle'
  && Boolean(Object.keys(state.config || {}).length)
  && dismissedSessionId.value !== currentSessionId.value)
const sessionStatusLabel = computed(() => {
  if (state.config?.mode === 'replay') {
    if (state.status === 'paused') return 'Replay paused'
    if (state.status === 'running') return 'Replay running'
    return state.status === 'error' ? 'Replay failed' : 'Replay stopped'
  }
  if (state.status === 'paused') return 'Session paused'
  if (state.status === 'running') return 'Session live'
  return state.status === 'error' ? 'Session failed' : 'Session stopped'
})
const updates = computed(() => state.updates || [])
const selectedStrategyName = ref('')
const strategyNames = computed(() => {
  const configured = Array.isArray(state.config?.strategies) && state.config.strategies.length
    ? state.config.strategies
    : state.config?.strategy
      ? [state.config.strategy]
      : Object.keys(state.strategies || {})
  return [...new Set(configured.map(String).filter(name => name && name !== 'Monitor'))]
})
const activeStrategyName = computed(() =>
  strategyNames.value.includes(selectedStrategyName.value)
    ? selectedStrategyName.value
    : strategyNames.value[0] || '')
const activeStrategySnapshot = computed(() =>
  state.strategies?.[activeStrategyName.value] || snapshot.value)
const activeTradingConfig = computed(() => state.config?.config || {})
const riskMetrics = computed(() => {
  const tradeCount = activeStrategySnapshot.value.metrics?.n_trades
  const metrics = [
    {
      key: 'completed-trades', label: 'Completed trades', icon: ArrowRightLeft,
      value: Number.isFinite(Number(tradeCount))
        ? Number(tradeCount).toLocaleString('en', { maximumFractionDigits: 0 })
        : '—',
      help: 'Number of completed trades recorded for the selected strategy in this session.'
    },
    {
      key: 'gross-exposure', label: 'Gross exposure', icon: ChartNoAxesCombined,
      value: money(activeStrategySnapshot.value.gross_exposure),
      help: 'Total market value of all open positions, adding long and short positions without offsetting them.'
    }
  ]
  if (activeTradingConfig.value.allow_short) {
    metrics.push({
      key: 'net-exposure', label: 'Net exposure', icon: Scale,
      value: money(activeStrategySnapshot.value.net_exposure),
      help: 'Signed market value of open positions: long exposure minus short exposure.'
    })
  }
  if (activeTradingConfig.value.allow_margin) {
    metrics.push({
      key: 'leverage', label: 'Leverage', icon: Gauge,
      value: `${Number(activeStrategySnapshot.value.leverage || 0).toFixed(2)}x`,
      help: 'Gross exposure divided by account equity. 1.00x means gross positions equal current equity.'
    })
  }
  metrics.push(
    {
      key: 'buying-power', label: 'Buying power', icon: WalletCards,
      value: money(activeStrategySnapshot.value.buying_power),
      help: activeTradingConfig.value.allow_margin
        ? 'Additional gross exposure available before reaching configured leverage or margin limits.'
        : 'Account value still available for additional positions without borrowing funds.'
    },
    {
      key: 'drawdown', label: 'Drawdown', icon: TrendingDown,
      value: percent(activeStrategySnapshot.value.drawdown),
      help: 'Percentage change from the highest equity reached. A negative value shows how far equity is below its peak.'
    },
    {
      key: 'total-costs', label: 'Total costs', icon: ReceiptText,
      value: money(activeStrategySnapshot.value.total_costs),
      help: 'Cumulative simulated commissions and financing costs charged during this session.'
    }
  )
  return metrics
})
const strategyUpdates = computed(() => updates.value.map(update => {
  const strategyUpdate = update.strategies?.[activeStrategyName.value]
  if (!strategyUpdate) return update
  return {
    ...update,
    fills: strategyUpdate.fills || [],
    indicators: strategyUpdate.indicators || {},
    snapshot: strategyUpdate.snapshot || {}
  }
}))
const latestIndicators = computed(() => {
  const indicators = strategyUpdates.value.at(-1)?.indicators || {}
  const values = []
  for (const [name, symbols] of Object.entries(indicators)) {
    for (const [symbol, outputs] of Object.entries(symbols || {})) {
      const latest = (outputs || []).map(output => Array.isArray(output) ? output.at(-1) : output)
      values.push({
        key: `indicator:${name}:${symbol}`,
        name,
        symbol,
        value: latest.map(value => Number(value).toLocaleString('en', {
          maximumFractionDigits: 6
        })).join(', ')
      })
    }
  }
  return values
})
const liveMetrics = computed(() => {
  const configured = Array.isArray(state.config?.config?.metrics)
    ? state.config.config.metrics
    : form.config.metrics
  const values = activeStrategySnapshot.value.metrics || {}
  return configured
    .filter(metric => metric !== 'n_trades')
    .map(metric => ({
      key: metric,
      metric,
      label: metricName(metric),
      value: Object.hasOwn(values, metric) ? values[metric] : null
    }))
})
const fills = computed(() =>
  state.recent_order_outcomes?.[activeStrategyName.value]
  ?? flattenFills(strategyUpdates.value, 12))
const executionEmptyMessage = computed(() => {
  if (!hasStrategy.value) return 'Monitoring only · no strategy selected.'
  if (Object.keys(activeStrategySnapshot.value.portfolio?.positions || {}).length) {
    return 'No recent orders. Older completed orders can leave this bounded list while positions remain open.'
  }
  return 'Waiting for strategy orders…'
})
const replayStatus = computed(() => state.replay || state.health?.replay || {})
const replayProgress = computed(() => Math.min(100, Math.max(
  0,
  Number(replayStatus.value.progress || 0) * 100
)))
const replaySpeedLabel = computed(() => Number(replayStatus.value.speed) > 0
  ? `${Number(replayStatus.value.speed)}× playback`
  : 'Maximum-speed playback')
const replayWarmupLabel = computed(() => {
  const count = Number(replayStatus.value.warmup_bars_loaded || 0)
  if (replayStatus.value.warmup_source === 'recorded') {
    return `Restored the original ${count} warm-up bars.`
  }
  if (replayStatus.value.warmup_source === 'storage') {
    return `Loaded ${count} warm-up bars from current local storage.`
  }
  return 'No recorded warm-up data was available for this session.'
})
const replayDurationLabel = computed(() => {
  const seconds = Number(replayStatus.value.source_duration_seconds || 0)
  if (seconds < 60) return `${seconds.toFixed(1)}s recorded time`
  const minutes = Math.floor(seconds / 60)
  const remainder = Math.round(seconds % 60)
  return `${minutes}m ${remainder}s recorded time`
})
const initialEquity = computed(() =>
  Number(state.config?.config?.initial_cash ?? form.config.initial_cash) || 0)
const baseCurrency = computed(() => sessionVisible.value
  ? state.config?.config?.base_currency || form.config.base_currency
  : form.config.base_currency)
const watchlist = computed(() => {
  const prices = snapshot.value.latest_prices || {}
  const symbols = state.config?.symbols?.length ? state.config.symbols : Object.keys(prices)
  return [...new Set(symbols)].map(symbol => ({ symbol, price: prices[symbol] }))
})
const disabledProviderIntervals = computed(() =>
  intervals.filter(interval => !available(form.provider, interval))
)
const equitySeries = computed(() => sessionEquitySeries(strategyUpdates.value).filter(
  item => item.equity !== null && Number.isFinite(Number(item.equity))
))
const marketSeries = computed(() => {
  const series = new Map()
  for (const update of updates.value) {
    const market = update.market || {}
    const symbol = String(market.symbol || '')
    const timestamp = Number(market.received_ts || market.close_ts)
    if (!symbol || !Number.isFinite(timestamp) || !Number.isFinite(Number(market.close))) continue
    if (!series.has(symbol)) series.set(symbol, [])
    series.get(symbol).push({ ...market, timestamp })
  }
  return series
})
const hasStrategy = computed(() => Boolean(state.config?.strategy || state.config?.strategies?.length))
const activeLivePlot = computed(() => livePlotTabs.find(item => item.id === livePlot.value) || livePlotTabs[0])
const liveChartEyebrow = computed(() => hasStrategy.value
  ? activeLivePlot.value.eyebrow
  : livePlotTabs[0].eyebrow)
const liveChartTitle = computed(() => hasStrategy.value
  ? activeLivePlot.value.title
  : livePlotTabs[0].title)
function strategySnapshotPoints(field) {
  return strategyUpdates.value
    .map(update => ({
      timestamp: Number(update.market?.received_ts || update.market?.close_ts || 0),
      value: update.snapshot?.[field]
    }))
    .filter(item => Number.isFinite(item.timestamp) && item.timestamp > 0 && Number.isFinite(Number(item.value)))
}
const liveChartMessage = computed(() => {
  if (!hasStrategy.value || livePlot.value === 'price') {
    return marketSeries.value.size ? '' : 'Waiting for the first WebSocket OHLC update…'
  }
  if (livePlot.value === 'pnl' || livePlot.value === 'equity') {
    return equitySeries.value.length ? '' : 'Waiting for the first account update…'
  }
  if (livePlot.value === 'exposure') {
    return strategySnapshotPoints('gross_exposure').length
      ? ''
      : 'Waiting for the first exposure update…'
  }
  return strategySnapshotPoints('drawdown').length
    ? ''
    : 'Waiting for the first drawdown update…'
})
const liveFigure = computed(() => {
  if (liveChartMessage.value) return null
  if (!hasStrategy.value || livePlot.value === 'price') {
    const data = [...marketSeries.value.entries()].map(([symbol, points]) => {
      const decimals = currencyDecimals(currencyForSymbol(symbol))
      return {
        type: 'scatter',
        mode: 'lines',
        name: symbol,
        x: points.map(item => new Date(item.timestamp * 1000)),
        y: points.map(item => Number(item.close)),
        customdata: points.map(item => [
          item.open,
          item.high,
          item.low,
          item.close,
          item.volume,
          item.is_final ? 'Closed candle' : 'Partial candle'
        ]),
        line: { width: 2 },
        hovertemplate: [
          '%{customdata[5]}',
          `Open %{customdata[0]:,.${decimals}f}`,
          `High %{customdata[1]:,.${decimals}f}`,
          `Low %{customdata[2]:,.${decimals}f}`,
          `Close %{customdata[3]:,.${decimals}f}`,
          'Volume %{customdata[4]:,.4f}',
          '<extra>%{fullData.name}</extra>'
        ].join('<br>')
      }
    })
    const firstSymbol = marketSeries.value.keys().next().value
    const priceDecimals = currencyDecimals(currencyForSymbol(firstSymbol))
    return {
      data,
      layout: {
        yaxis: { title: 'Price', tickformat: `,.${priceDecimals}f` },
        xaxis: { title: '', tickformat: configuredPlotlyDateTimeFormat(props.bootstrap.display) },
        showlegend: data.length > 1
      }
    }
  }
  const currency = baseCurrency.value
  const decimals = currencyDecimals(currency)
  if (livePlot.value === 'equity') {
    return {
      data: [{
        type: 'scatter',
        mode: 'lines',
        name: 'Equity',
        x: equitySeries.value.map(item => new Date(item.timestamp * 1000)),
        y: equitySeries.value.map(item => Number(item.equity)),
        line: { color: '#5ba1ff', width: 2 },
        hovertemplate: `${currency} %{y:,.${decimals}f}<extra>Equity</extra>`
      }],
      layout: {
        yaxis: { title: `Equity (${currency})`, tickformat: `,.${decimals}f` },
        xaxis: { title: '', tickformat: configuredPlotlyDateTimeFormat(props.bootstrap.display) },
        showlegend: false
      }
    }
  }
  if (livePlot.value === 'exposure') {
    const gross = strategySnapshotPoints('gross_exposure')
    const net = strategySnapshotPoints('net_exposure')
    const data = [{
      type: 'scatter', mode: 'lines', name: 'Gross exposure',
      x: gross.map(item => new Date(item.timestamp * 1000)),
      y: gross.map(item => Number(item.value)),
      line: { color: '#f6bd55', width: 2 },
      hovertemplate: `${currency} %{y:,.${decimals}f}<extra>Gross exposure</extra>`
    }]
    if (activeTradingConfig.value.allow_short && net.length) {
      data.push({
        type: 'scatter', mode: 'lines', name: 'Net exposure',
        x: net.map(item => new Date(item.timestamp * 1000)),
        y: net.map(item => Number(item.value)),
        line: { color: '#5ba1ff', width: 2 },
        hovertemplate: `${currency} %{y:,.${decimals}f}<extra>Net exposure</extra>`
      })
    }
    return {
      data,
      layout: {
        yaxis: { title: `Exposure (${currency})`, tickformat: `,.${decimals}f`, zeroline: true },
        xaxis: { title: '', tickformat: configuredPlotlyDateTimeFormat(props.bootstrap.display) },
        showlegend: data.length > 1
      }
    }
  }
  if (livePlot.value === 'drawdown') {
    const drawdown = strategySnapshotPoints('drawdown')
    return {
      data: [{
        type: 'scatter',
        mode: 'lines',
        name: 'Drawdown',
        x: drawdown.map(item => new Date(item.timestamp * 1000)),
        y: drawdown.map(item => Number(item.value) * 100),
        line: { color: '#f15b64', width: 2 },
        hovertemplate: '%{y:.2f}%<extra>Drawdown</extra>'
      }],
      layout: {
        yaxis: { title: 'Drawdown (%)', ticksuffix: '%', zeroline: true },
        xaxis: { title: '', tickformat: configuredPlotlyDateTimeFormat(props.bootstrap.display) },
        showlegend: false
      }
    }
  }
  return {
    data: [{
      type: 'scatter',
      mode: 'lines',
      name: 'Net P&L',
      x: equitySeries.value.map(item => new Date(item.timestamp * 1000)),
      y: equitySeries.value.map(item => Number(item.equity) - initialEquity.value),
      line: { color: '#23c483', width: 2 },
      hovertemplate: `${currency} %{y:,.${decimals}f}<extra>Net P&L</extra>`
    }],
    layout: {
      yaxis: { title: `Net P&L (${currency})`, tickformat: `,.${decimals}f`, zeroline: true },
      xaxis: { title: '', tickformat: configuredPlotlyDateTimeFormat(props.bootstrap.display) },
      showlegend: false
    }
  }
})
function title(value) { return value.charAt(0).toUpperCase() + value.slice(1) }
function defaultLiveForm() {
  return {
    provider: 'kraken',
    interval: '1m',
    symbols: ['BTC-USD'],
    strategies: [],
    indicators: [],
    warmup_bars: 500,
    config: {
      initial_cash: portfolioDefaults.initial_cash ?? 10000,
      base_currency: portfolioDefaults.base_currency || 'USD',
      commission_pct: 0.05,
      commission_fixed: 0,
      slippage: 0.01,
      allow_short: false,
      allow_margin: false,
      trade_on_partial: false,
      max_history: 10000,
      max_leverage: 2,
      initial_margin: 50,
      maintenance_margin: 25,
      margin_interest: 0,
      borrow_rate: 0,
      max_position_size: 100,
      max_drawdown: 0,
      allowed_order_types: ['Market', 'Limit', 'StopLoss', 'TakeProfit', 'StopLossLimit', 'TakeProfitLimit', 'TrailingStop', 'TrailingStopLimit', 'SettlePosition', 'Cancel'],
      partial_fills: false,
      max_volume_participation: 100,
      metrics: ['total_return', 'pnl', 'final_equity', 'win_rate', 'ann_volatility', 'sharpe', 'sortino', 'max_dd'],
      risk_free_rate: 0
    }
  }
}
function applyExperimentDraft() {
  let draft = null
  try {
    draft = JSON.parse(sessionStorage.getItem('backtide:session-config') || 'null')
  } catch {
    draft = null
  }
  sessionStorage.removeItem('backtide:session-config')
  if (!draft || typeof draft !== 'object' || Array.isArray(draft)) return false
  const defaults = defaultLiveForm()
  const draftConfig = { ...defaults.config, ...(draft.config || {}) }
  Object.assign(form, defaults, draft, {
    config: draftConfig
  })
  setupTab.value = 0
  return true
}
function capability(provider, interval = form.interval) {
  const value = props.bootstrap.live.providers?.[provider]
  if (Array.isArray(value)) return { supported: value[0], reason: value[1] }
  if (!value || typeof value !== 'object') return { supported: false, reason: String(value || '') }
  return value.intervals?.[interval] || value
}
function providerAvailable(provider) {
  const value = props.bootstrap.live.providers?.[provider]
  return Array.isArray(value) ? Boolean(value[0]) : Boolean(value?.supported)
}
function available(provider, interval = form.interval) { return Boolean(capability(provider, interval).supported) }
async function selectProvider(provider) {
  if (!providerAvailable(provider)) return
  form.provider = provider
  form.symbols = []
  if (!available(provider)) {
    form.interval = intervals.find(interval => available(provider, interval)) || form.interval
  }
  await loadLiveInstruments()
}
async function loadLiveInstruments() {
  const request = ++liveCatalogRequest
  const provider = form.provider
  loadingLiveSymbols.value = true
  liveSymbolError.value = ''
  try {
    const result = await query('/api/live/instruments', {
      provider,
      limit: 10000
    })
    if (request !== liveCatalogRequest) return
    liveInstruments.value = [...result].sort((left, right) =>
      left.symbol.localeCompare(right.symbol))
  } catch (error) {
    if (request !== liveCatalogRequest) return
    liveInstruments.value = []
    liveSymbolError.value = `Could not load ${title(provider)} symbols. ${error.message}`
    emit('toast', liveSymbolError.value, 'error')
  } finally {
    if (request === liveCatalogRequest) loadingLiveSymbols.value = false
  }
}
function setBaseCurrency(value) { form.config.base_currency = value }
function moveLiveMetric(key, direction) {
  const from = form.config.metrics.indexOf(key)
  const to = from + direction
  if (from < 0 || to < 0 || to >= form.config.metrics.length) return
  const reordered = [...form.config.metrics]
  reordered.splice(to, 0, reordered.splice(from, 1)[0])
  form.config.metrics = reordered
}
function clearLiveMetrics() {
  finishLiveMetricDrag()
  form.config.metrics = []
}
function removeLiveMetric(key) {
  if (draggedMetricKey.value === key) finishLiveMetricDrag()
  form.config.metrics = form.config.metrics.filter(metric => metric !== key)
}
function startLiveMetricDrag(event, key) {
  draggedMetricKey.value = key
  dragOverMetricKey.value = ''
  if (!event.dataTransfer) return
  event.dataTransfer.effectAllowed = 'move'
  event.dataTransfer.setData('text/plain', key)
  if (typeof event.dataTransfer.setDragImage !== 'function') return
  const bounds = event.currentTarget.getBoundingClientRect()
  const offsetX = Math.max(0, Math.min(bounds.width, event.clientX - bounds.left))
  const offsetY = Math.max(0, Math.min(bounds.height, event.clientY - bounds.top))
  metricDragPreview?.remove()
  metricDragPreview = event.currentTarget.cloneNode(true)
  metricDragPreview.classList.remove('dragging', 'drop-target')
  metricDragPreview.classList.add('metric-drag-preview')
  metricDragPreview.style.width = `${bounds.width}px`
  document.body.append(metricDragPreview)
  event.dataTransfer.setDragImage(metricDragPreview, offsetX, offsetY)
}
function dragOverLiveMetric(event, targetKey) {
  const sourceKey = draggedMetricKey.value
  if (!sourceKey || targetKey === sourceKey) return
  dragOverMetricKey.value = targetKey
  const from = form.config.metrics.indexOf(sourceKey)
  const target = form.config.metrics.indexOf(targetKey)
  if (from < 0 || target < 0) return
  const bounds = event.currentTarget.getBoundingClientRect()
  let insertion = target + (event.clientY > bounds.top + bounds.height / 2 ? 1 : 0)
  if (from < insertion) insertion -= 1
  insertion = Math.max(0, Math.min(form.config.metrics.length - 1, insertion))
  if (insertion === from) return
  const reordered = [...form.config.metrics]
  const [metric] = reordered.splice(from, 1)
  reordered.splice(insertion, 0, metric)
  form.config.metrics = reordered
}
function finishLiveMetricDrag() {
  metricDragPreview?.remove()
  metricDragPreview = null
  draggedMetricKey.value = ''
  dragOverMetricKey.value = ''
}
function updateState(next) {
  Object.assign(state, next)
  emit('live-status', state)
}
function symbolLogo(symbol) {
  if (!symbol || failedSymbolLogos.value.has(symbol)) return ''
  return instrumentLogoUrl(symbol, 'Crypto', props.bootstrap.display.logokit_api_key)
}
function markSymbolLogoFailed(symbol) {
  if (symbol) failedSymbolLogos.value = new Set(failedSymbolLogos.value).add(symbol)
}
function fillKey(fill, index) { return `${fill.order?.id || ''}:${fill.timestamp}:${fill.status}:${index}` }
function fillTime(fill) {
  return formatConfiguredTimeWithSeconds(fill.timestamp, props.bootstrap?.display)
}
function fillSide(fill) {
  if (fill.order?.quantity == null) return '—'
  const quantity = Number(fill.order?.quantity)
  return quantity > 0 ? 'Buy' : quantity < 0 ? 'Sell' : '—'
}
function fillSideClass(fill) {
  const side = fillSide(fill)
  return side === 'Buy' ? 'positive' : side === 'Sell' ? 'negative' : ''
}
function fillQuantity(fill) {
  if (fill.order?.quantity == null) return '—'
  const quantity = Number(fill.order?.quantity)
  return Number.isFinite(quantity) ? positionAmount(Math.abs(quantity)) : '—'
}
function fillOrderType(fill) {
  const value = String(fill.order?.order_type || '').trim()
  return value ? title(value.replace(/([a-z])([A-Z])/g, '$1 $2').replaceAll('_', ' ')) : '—'
}
function realizedTradePnl(fill) {
  if (fill.realized_pnl == null) return null
  const pnl = Number(fill.realized_pnl)
  const commission = Number(fill.commission)
  if (!Number.isFinite(pnl)) return null
  return pnl + (Number.isFinite(commission) ? commission : 0)
}
function currencyDecimals(currency = baseCurrency.value || 'USD') {
  return configuredCurrencyDecimals(currency, props.bootstrap?.enums?.currencies)
}
function currencyForSymbol(symbol) {
  const quote = String(symbol || '').toUpperCase().split(/[-/:]/).filter(Boolean).at(-1)
  return props.bootstrap?.enums?.currencies?.some(item => item.code === quote)
    ? quote
    : baseCurrency.value || 'USD'
}
function money(value, currency = baseCurrency.value || 'USD') {
  return formatConfiguredCurrency(
    value,
    currency,
    props.bootstrap.display,
    props.bootstrap?.enums?.currencies
  )
}
function positionAmount(value) { return Number(value).toLocaleString('en', { maximumFractionDigits: 8 }) }
function price(value, symbol) {
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) return '—'
  const decimals = currencyDecimals(currencyForSymbol(symbol))
  return parsed.toLocaleString('en', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals
  })
}
function percent(value) { return `${(Number(value) * 100 || 0).toFixed(2)}%` }
function metricName(key) { return liveMetricCatalog.value.find(item => item.key === key)?.name || String(key).replaceAll('_', ' ') }
function metricValue(key, value) {
  if (value == null) return '—'
  if (liveMetricCatalog.value.find(item => item.key === key)?.percentage) return percent(value)
  if (currencyMetrics.has(key)) return money(value)
  return Number(value).toLocaleString('en', { maximumFractionDigits: 4 })
}
const tone = value => Number(value) > 0 ? 'positive' : Number(value) < 0 ? 'negative' : ''
function eventTime(update) {
  const value = update?.received_at ?? update?.market?.received_ts ?? update?.market?.close_ts
  return formatConfiguredTimeWithSeconds(value, props.bootstrap?.display, 'now')
}
async function start() {
  starting.value = true
  const request = {
    ...form,
    config: {
      ...form.config,
      metrics: [...new Set([
        ...form.config.metrics.filter(metric => metric !== 'n_trades'),
        'n_trades'
      ])]
    }
  }
  try {
    updateState(await post('/api/live', request))
    emit('toast', 'Live session started.')
    poll()
  } catch (error) {
    emit('toast', error.message, 'error')
  } finally {
    starting.value = false
  }
}
async function stop() { try { updateState(await post('/api/live/stop')); clearTimeout(timer); emit('toast', 'Live session stopped.') } catch (error) { emit('toast', error.message, 'error') } }
function newConfiguration() {
  dismissedSessionId.value = currentSessionId.value
  updateState({
    id: null,
    status: 'idle',
    config: {},
    snapshot: {},
    strategies: {},
    updates: [],
    recent_order_outcomes: {},
    health: {},
    replay: null,
    error: null
  })
  Object.assign(form, defaultLiveForm())
  setupTab.value = 0
  selectedStrategyName.value = ''
  loadLiveInstruments()
}
function openSessionHistory() { emit('navigate', 'live-history') }
async function pause() { try { updateState(await post('/api/live/pause')); emit('toast', 'Strategy evaluation paused.') } catch (error) { emit('toast', error.message, 'error') } }
async function resume() { try { updateState(await post('/api/live/resume')); emit('toast', 'Strategy evaluation resumed.'); poll() } catch (error) { emit('toast', error.message, 'error') } }
async function flatten() { try { updateState(await post('/api/live/flatten')); emit('toast', 'Positions will flatten on the next market update.', 'warning') } catch (error) { emit('toast', error.message, 'error') } }
async function cancelAll() { try { updateState(await post('/api/live/cancel-all')); emit('toast', 'Open orders will cancel on the next market update.', 'warning') } catch (error) { emit('toast', error.message, 'error') } }
async function poll() { try { updateState(await api('/api/live')) } catch (error) { state.error = error.message } if (activeSession.value) timer = setTimeout(poll, 1000) }
async function initialize() {
  try {
    updateState(await api('/api/live'))
    if (activeSession.value) poll()
  } catch (error) {
    state.error = error.message
    emit('toast', error.message, 'error')
  }
}
onMounted(() => {
  initialize()
  loadLiveInstruments()
})
let activationCount = 0
onActivated(() => {
  const promoted = applyExperimentDraft()
  if (activationCount++) {
    initialize()
    if (promoted) loadLiveInstruments()
  }
})
onDeactivated(() => clearTimeout(timer))
onBeforeUnmount(() => {
  clearTimeout(timer)
  finishLiveMetricDrag()
})
</script>
