const $ = (id) => document.getElementById(id);
let worker;
let timer;
let urls = [];
let lastInput;

function stop() {
  clearTimeout(timer);
  worker?.terminate();
  worker = undefined;
  $('cancel').disabled = true;
  $('canvas').setAttribute('aria-busy', 'false');
}
function status(message, error = false) {
  $('status').textContent = message;
  $('status').classList.toggle('error', error);
}
function clearPreview() {
  $('save').disabled = true;
  $('save-svg').disabled = true;
  $('preview').hidden = true;
  $('preview').removeAttribute('src');
  $('placeholder').hidden = false;
  for (const id of ['png', 'svg']) { $(id).hidden = true; $(id).removeAttribute('href'); }
  urls.forEach((url) => URL.revokeObjectURL(url));
  urls = [];
  $('warnings').hidden = true;
  $('dimensions').textContent = '';
}
function run(input, label = 0) {
  stop();
  clearPreview();
  $('label').disabled = true;
  if (!globalThis.Worker || !globalThis.WebAssembly) {
    status('This browser needs WebAssembly and module worker support.', true); return;
  }
  if (new TextEncoder().encode(input.source).length > 1_048_576) {
    status('ZPL exceeds the 1 MiB input limit.', true); return;
  }
  status('Rendering locally…');
  $('canvas').setAttribute('aria-busy', 'true');
  $('cancel').disabled = false;
  const fail = (message) => { stop(); status(message, true); };
  try {
    worker = new Worker(new URL('./worker.mjs', import.meta.url), { type: 'module' });
    worker.onerror = () => fail('Renderer failed to load or run. Try again, or use a current browser.');
    worker.onmessage = ({ data }) => {
      stop();
      if (data.error) { status(data.error, true); return; }
      urls = [URL.createObjectURL(new Blob([data.png], { type: 'image/png' })),
        URL.createObjectURL(new Blob([data.svg], { type: 'image/svg+xml' }))];
      $('preview').src = urls[0];
      $('preview').width = data.width;
      $('preview').height = data.height;
      $('preview').hidden = false;
      $('placeholder').hidden = true;
      for (const [i, id] of ['png', 'svg'].entries()) {
        $(id).href = urls[i]; $(id).download = `label-${label + 1}.${id}`;
      }
      $('save').disabled = false;
      $('save-svg').disabled = false;
      $('label').max = data.labels;
      $('label').value = label + 1;
      $('label').disabled = data.labels < 2;
      $('dimensions').textContent = `${data.width} × ${data.height} dots`;
      // Omit the general printer-fidelity caveat from this compact demo.
      const warnings = data.warnings.split('\n').filter(message => message !==
        'Resident fonts use captured bitmap strikes; unsampled sizes, resolutions or rotations can differ from printer rasterization.').join('\n');
      $('warnings').textContent = warnings;
      $('warnings').hidden = !warnings;
      status(`Label ${label + 1} of ${data.labels}. Rendered locally.`);
    };
    timer = setTimeout(() => fail('Rendering exceeded 20 seconds. Simplify the label or reduce its dimensions.'), 20_000);
    worker.postMessage({ ...input, label });
  } catch (error) { fail(error.message); }
}
$('editor').addEventListener('submit', (event) => {
  event.preventDefault();
  lastInput = { source: $('source').value, width: Number($('width').value),
    height: Number($('height').value), dpi: Number($('dpi').value) };
  run(lastInput);
});
$('editor').addEventListener('input', () => {
  stop(); clearPreview(); lastInput = undefined; $('label').disabled = true;
  status('Source or settings changed. Render to update the preview.');
});
$('label').addEventListener('change', () => {
  if (lastInput && $('label').reportValidity()) run(lastInput, Number($('label').value) - 1);
});
$('save-svg').addEventListener('click', () => { if (!$('save-svg').disabled) $('svg').click(); });
$('save').addEventListener('click', () => { if (!$('save').disabled) $('png').click(); });
$('cancel').addEventListener('click', () => { stop(); status('Rendering cancelled.'); });
window.addEventListener('pagehide', () => { stop(); clearPreview(); });
