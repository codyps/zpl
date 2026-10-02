const $ = (id) => document.getElementById(id);
let worker;
let timer;
let urls = [];
let currentPrint;
let displayed;
let operation = 0;
let lastPaper;
let outgoingUrl;
const pendingUrls = new Set();
const history = [];
let historyBytes = 0;
let nextPrint = 1;
const MAX_HISTORY = 20;
const MAX_HISTORY_BYTES = 64 * 1024 * 1024;

function stop() {
  operation++;
  clearTimeout(timer);
  worker?.terminate();
  worker = undefined;
  $('cancel').disabled = true;
  $('canvas').setAttribute('aria-busy', 'false');
}
function status(message, error = false) {
  $('status').textContent = message;
  const note = $('error-paper');
  note.getAnimations().forEach(animation => animation.cancel());
  note.hidden = !error;
  $('canvas').classList.toggle('has-error', error);
  $('error-message').textContent = error ? message : '';
  if (error && !matchMedia('(prefers-reduced-motion: reduce)').matches) {
    const travel = $('canvas').clientHeight;
    note.animate([
      { transform: `translate(-50%, calc(-50% - ${travel}px)) rotate(-3deg)` },
      { transform: 'translate(-50%, -50%) rotate(-1deg)' },
    ], { duration: 360, easing: 'cubic-bezier(.2,.7,.2,1)' });
  }
}
function clearFeed() {
  pendingUrls.forEach(url => URL.revokeObjectURL(url));
  pendingUrls.clear();
  for (const id of ['preview', 'previous-preview', 'printer-slot']) {
    $(id).getAnimations().forEach(animation => animation.cancel());
  }
  $('paper-feed').hidden = true;
  $('previous-preview').removeAttribute('src');
  if (outgoingUrl) URL.revokeObjectURL(outgoingUrl);
  outgoingUrl = undefined;
  $('preview').classList.remove('awaiting-feed');
}
function markStale(stale) {
  $('canvas').classList.toggle('is-stale', stale);
  $('preview-state').hidden = !stale;
  if (stale) $('preview').setAttribute('aria-describedby', 'preview-state');
  else $('preview').removeAttribute('aria-describedby');
}

function clearPreview(keepPaper = false) {
  clearFeed();
  $('preview').onload = null;
  $('save').disabled = true;
  $('save-svg').disabled = true;
  $('save-pdf').disabled = true;
  if (!keepPaper) {
    displayed = undefined;
    markStale(false);
    $('preview').hidden = true;
    $('preview').removeAttribute('src');
    $('placeholder').hidden = false;
  }
  for (const id of ['png', 'svg', 'pdf']) { $(id).hidden = true; $(id).removeAttribute('href'); }
  if (!keepPaper) {
    urls.forEach((url) => URL.revokeObjectURL(url));
    urls = [];
  }
  if (!keepPaper) $('warnings').hidden = true;
  if (!keepPaper) {
    $('dimensions').textContent = '';
    $('history').value = '';
    $('print-id').hidden = true;
  }
  $('previous-label').disabled = true;
  $('next-label').disabled = true;
}

async function animatePrint(previous) {
  const image = $('preview');
  const source = image.src;
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
    image.classList.remove('awaiting-feed');
    return;
  }
  const paper = image.getBoundingClientRect();
  const canvas = $('canvas').getBoundingClientRect();
  const timing = { duration: 420, easing: 'cubic-bezier(.2,.7,.2,1)' };
  // Move both complete sheets through the canvas viewport, with a visible gap.
  // Keep the outgoing sheet at its original fitted size when dimensions change.
  // https://developer.mozilla.org/en-US/docs/Web/API/Element/animate
  const outgoing = $('previous-preview').getBoundingClientRect();
  const travel = Math.max(paper.bottom - canvas.top + 8,
    previous ? canvas.bottom - outgoing.top + 8 : 0,
    previous ? paper.bottom - outgoing.top + 16 : 0);
  const animations = [];
  if (previous) {
    $('paper-feed').hidden = false;
    animations.push($('previous-preview').animate([
      { transform: 'translateY(0px)' }, { transform: `translateY(${travel}px)` },
    ], timing));
  }
  const incoming = image.animate([
    { transform: `translate(-50%, calc(-50% - ${travel}px))` },
    { transform: 'translate(-50%, -50%)' },
  ], timing);
  animations.push(incoming);
  animations.push($('printer-slot').animate([
    { opacity: 0 }, { opacity: 1, offset: 0.1 }, { opacity: 1, offset: 0.8 }, { opacity: 0 },
  ].map(frame => ({ ...frame, left: `${paper.left - canvas.left - 5}px`,
    top: `0px`, width: `${paper.width + 8}px` })), { duration: timing.duration }));
  // Explicitly share a timeline start so the sheets cannot drift apart.
  const start = document.timeline.currentTime;
  animations.forEach(animation => { animation.startTime = start; });
  image.classList.remove('awaiting-feed');
  try { await incoming.finished; } catch { return; }
  if (image.src === source) clearFeed();
}

async function showPreview(job, data, label, animate = false, ready = () => {}, shown = () => {}) {
  const ticket = operation;
  const previous = lastPaper;
  const play = animate && !matchMedia('(prefers-reduced-motion: reduce)').matches;
  const paperBlob = new Blob([data.png], { type: 'image/png' });
  const nextUrls = [URL.createObjectURL(paperBlob),
    URL.createObjectURL(new Blob([data.svg], { type: 'image/svg+xml' })),
    URL.createObjectURL(new Blob([data.pdf], { type: 'application/pdf' }))];
  const oldUrl = play && previous ? URL.createObjectURL(previous) : undefined;
  const preparedUrls = oldUrl ? [...nextUrls, oldUrl] : nextUrls;
  preparedUrls.forEach(url => pendingUrls.add(url));
  // Decode off-screen before replacing either visible sheet. Loading must not
  // expose a blank frame between the settled image and the paper animation.
  // https://developer.mozilla.org/en-US/docs/Web/API/HTMLImageElement/decode
  const decode = async (src) => {
    const image = new Image();
    image.src = src;
    await image.decode();
  };
  try { await Promise.all([decode(nextUrls[0]), ...(oldUrl ? [decode(oldUrl)] : [])]); }
  catch {
    preparedUrls.forEach(url => { pendingUrls.delete(url); URL.revokeObjectURL(url); });
    if (ticket === operation) { stop(); status('Preview image could not be decoded. Render to try again.', true); }
    return;
  }
  if (ticket !== operation) {
    preparedUrls.forEach(url => { pendingUrls.delete(url); URL.revokeObjectURL(url); });
    return;
  }
  preparedUrls.forEach(url => pendingUrls.delete(url));
  if (oldUrl) {
    outgoingUrl = oldUrl;
    $('previous-preview').src = oldUrl;
    // Cover the existing image before changing its src. The animation will
    // reposition this sheet if the new label has different dimensions.
    if (!$('preview').hidden) {
      const paper = $('preview').getBoundingClientRect();
      const canvas = $('canvas').getBoundingClientRect();
      Object.assign($('paper-feed').style, {
        left: `${paper.left - canvas.left - $('canvas').clientLeft}px`,
        top: `${paper.top - canvas.top - $('canvas').clientTop}px`,
        width: `${paper.width}px`, height: `${paper.height}px`,
      });
      $('paper-feed').hidden = false;
    }
  }
  urls.forEach(url => URL.revokeObjectURL(url));
  lastPaper = paperBlob;
  urls = nextUrls;
  $('preview').classList.toggle('awaiting-feed', play);
  const source = urls[0];
  $('preview').onload = async () => {
    if (play) await animatePrint(previous);
    if ($('preview').src === source) ready();
  };
  $('preview').src = urls[0];
  $('preview').width = data.width;
  $('preview').height = data.height;
  $('preview').hidden = false;
  $('placeholder').hidden = true;
  for (const [i, id] of ['png', 'svg', 'pdf'].entries()) {
    $(id).href = urls[i]; $(id).download = `print-${job.id}-label-${label + 1}.${id}`;
  }
  $('save').disabled = false;
  $('save-svg').disabled = false;
  $('save-pdf').disabled = false;
  job.label = label;
  $('print-id').textContent = `Print #${job.id}`;
  $('print-id').hidden = false;
  $('preview').alt = `Print #${job.id}, label ${label + 1} of ${data.labels}`;
  $('label-total').textContent = `of ${data.labels}`;
  $('previous-label').disabled = label === 0;
  $('next-label').disabled = label + 1 >= data.labels;
  $('label').max = data.labels;
  $('label').value = label + 1;
  $('label').disabled = data.labels < 2;
  $('dimensions').textContent = `${data.width} × ${data.height} dots`;
  // Omit the general printer-fidelity caveat from this compact demo.
  const warnings = data.warnings.split('\n').filter(message => message !==
    'Resident fonts use captured bitmap strikes; unsampled sizes, resolutions or rotations can differ from printer rasterization.').join('\n');
  $('warnings').textContent = warnings;
  $('warnings').hidden = !warnings;
  displayed = { job, data, label };
  markStale(false);
  shown();
}

function updateHistory(selected = '') {
  const placeholder = new Option(`History (${history.length} prints)`, '');
  placeholder.disabled = true;
  $('history').replaceChildren(placeholder, ...history.map(job =>
    new Option(`Print #${job.id} · ${job.total} ${job.total === 1 ? 'label' : 'labels'} · ${job.time}`, String(job.id))));
  $('history').value = selected;
  $('history').disabled = history.length === 0;
  $('clear-history').disabled = history.length === 0;
}

function remember(job, label, data) {
  const bytes = data.png.byteLength + data.svg.byteLength + data.pdf.byteLength + data.warnings.length * 2;
  job.total = data.labels;
  if (job.forget || job.oversize || job.bytes + bytes > MAX_HISTORY_BYTES) {
    const index = history.indexOf(job);
    if (index !== -1) { history.splice(index, 1); historyBytes -= job.bytes; }
    job.pages.clear();
    job.pages.set(label, data);
    job.oversize = !job.forget;
    updateHistory();
    return;
  }
  if (!history.includes(job)) {
    history.unshift(job);
    historyBytes += job.bytes;
  }
  job.pages.set(label, data);
  job.bytes += bytes;
  historyBytes += bytes;
  while (history.length > MAX_HISTORY || historyBytes > MAX_HISTORY_BYTES) {
    const index = history.findLastIndex(entry => entry !== job);
    historyBytes -= history.splice(index, 1)[0].bytes;
  }
  updateHistory(String(job.id));
}

function run(job, label = 0, advance = false) {
  const keepPaper = !$('preview').hidden;
  stop();
  clearPreview(keepPaper);
  currentPrint = job;
  const ticket = operation;
  const input = job.input;
  $('label').disabled = true;
  const prefix = `Print #${job.id} · Label ${label + 1}`;
  const fail = (message) => { stop(); clearFeed(); status(`${prefix}: ${message}`, true); };
  const present = (data) => {
    showPreview(job, data, label, advance, () => {
      if (ticket !== operation) return;
      if (advance && label + 1 < data.labels) run(job, label + 1, true);
      else stop();
    }, () => {
      $('history').value = history.includes(job) ? String(job.id) : '';
      status('');
    });
  };
  $('cancel').disabled = false;
  $('canvas').setAttribute('aria-busy', 'true');
  if (job.pages.has(label)) { present(job.pages.get(label)); return; }
  if (!globalThis.Worker || !globalThis.WebAssembly) {
    fail('This browser needs WebAssembly and module worker support.'); return;
  }
  if (new TextEncoder().encode(input.source).length > 1_048_576) {
    fail('ZPL exceeds the 1 MiB input limit.'); return;
  }
  status('');
  try {
    const currentWorker = new Worker(new URL('./worker.mjs', import.meta.url), { type: 'module' });
    worker = currentWorker;
    worker.onerror = () => { if (ticket === operation) fail('Renderer failed to load or run. Try again, or use a current browser.'); };
    worker.onmessage = ({ data }) => {
      if (ticket !== operation) return;
      clearTimeout(timer);
      currentWorker.terminate();
      worker = undefined;
      if (data.error) { fail(data.error); return; }
      remember(job, label, data);
      present(data);
    };
    timer = setTimeout(() => fail('Rendering exceeded 20 seconds. Simplify the label or reduce its dimensions.'), 20_000);
    worker.postMessage({ ...input, label, printId: job.id });
  } catch (error) { fail(error.message); }
}
$('editor').addEventListener('submit', (event) => {
  event.preventDefault();
  const input = { source: $('source').value, width: Number($('width').value),
    height: Number($('height').value), dpi: Number($('dpi').value) };
  run({ id: nextPrint++, input, total: 0, label: 0, pages: new Map(),
    bytes: input.source.length * 2, oversize: false,
    time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) }, 0, true);
});
$('editor').addEventListener('input', () => {
  stop(); clearFeed();
  // Keep the last error visible while its source is being corrected.
  if (!$('error-paper').hidden) return;
  const matches = displayed && $('source').value === displayed.job.input.source &&
    ['width', 'height', 'dpi'].every(id => Number($(id).value) === displayed.job.input[id]);
  currentPrint = matches ? displayed.job : undefined;
  markStale(Boolean(displayed && !matches));
  $('save').disabled = !matches;
  $('save-svg').disabled = !matches;
  $('save-pdf').disabled = !matches;
  $('label').disabled = !matches || displayed.data.labels < 2;
  $('previous-label').disabled = !matches || displayed.label === 0;
  $('next-label').disabled = !matches || displayed.label + 1 >= displayed.data.labels;
  $('history').value = matches && history.includes(displayed.job) ? String(displayed.job.id) : '';
  if (matches) {
    // Undoing an edit restores the exact saved image and its metadata immediately.
    for (const [i, id] of ['png', 'svg', 'pdf'].entries()) {
      $(id).href = urls[i]; $(id).download = `print-${displayed.job.id}-label-${displayed.label + 1}.${id}`;
    }
  }
  status('');
});
$('label').addEventListener('change', () => {
  if (currentPrint && $('label').reportValidity()) run(currentPrint, Number($('label').value) - 1);
});
$('previous-label').addEventListener('click', () => {
  if (currentPrint && currentPrint.label > 0) run(currentPrint, currentPrint.label - 1);
});
$('next-label').addEventListener('click', () => {
  if (currentPrint && currentPrint.label + 1 < currentPrint.total) run(currentPrint, currentPrint.label + 1);
});
$('history').addEventListener('change', () => {
  const job = history.find(job => String(job.id) === $('history').value);
  if (!job) return;
  // Keep the current sheet and toolbar in place until the cached image is decoded.
  stop(); clearPreview(true);
  currentPrint = job;
  for (const id of ['source', 'width', 'height', 'dpi']) $(id).value = job.input[id];
  showPreview(job, job.pages.get(job.label), job.label);
  $('history').value = String(job.id);
  status('');
});
$('clear-history').addEventListener('click', () => {
  // Also stop automatic feeding so clearing history cannot immediately refill it.
  stop(); clearFeed();
  history.length = 0;
  historyBytes = 0;
  if (currentPrint) {
    const data = currentPrint.pages.get(currentPrint.label);
    currentPrint.pages.clear();
    currentPrint.bytes = currentPrint.input.source.length * 2;
    currentPrint.forget = true;
    if (data) currentPrint.pages.set(currentPrint.label, data);
  }
  updateHistory();
  status('');
});
$('save-pdf').addEventListener('click', () => { if (!$('save-pdf').disabled) $('pdf').click(); });
$('save-svg').addEventListener('click', () => { if (!$('save-svg').disabled) $('svg').click(); });
$('save').addEventListener('click', () => { if (!$('save').disabled) $('png').click(); });
$('cancel').addEventListener('click', () => { stop(); clearFeed(); status(''); });
window.addEventListener('pagehide', () => { stop(); clearPreview(); currentPrint = undefined; lastPaper = undefined; });
