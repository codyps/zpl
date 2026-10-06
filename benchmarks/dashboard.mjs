export function operations(records) {
  return [...new Set(records.flatMap(record => Object.keys(record.medians)))];
}

export function series(records, runner, operation) {
  const groups = new Map();
  for (const record of records.filter(r => r.runner === runner && Number.isFinite(r.medians[operation]))) {
    // Never connect points from different compilers, protocols, CPUs or images.
    const key = JSON.stringify([record.harness, record.environment]);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push({record, value: record.medians[operation]});
  }
  return [...groups.values()].map(points => points.sort((a, b) =>
    a.record.run_id - b.record.run_id || a.record.run_attempt - b.record.run_attempt));
}

export function draw(records, runner, operation, document) {
  const groups = series(records, runner, operation);
  const table = document.querySelector('tbody');
  const chart = document.querySelector('svg');
  table.replaceChildren();
  chart.replaceChildren();
  const values = groups.flat().map(p => p.value);
  if (!values.length) return;
  const max = Math.max(...values) * 1.1;
  const runs = [...new Set(groups.flat().sort((a, b) => a.record.run_id - b.record.run_id || a.record.run_attempt - b.record.run_attempt).map(p => `${p.record.run_id}/${p.record.run_attempt}`))];
  const colors = ['#087e8b', '#b23a48', '#6347b0', '#8c5700'];
  const svg = (tag, attrs) => {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
    chart.append(node);
    return node;
  };
  for (let i = 0; i <= 4; i++) {
    const y = 250 - i * 55;
    svg('line', {x1: 80, x2: 780, y1: y, y2: y, stroke: '#ddd'});
    svg('text', {x: 75, y: y + 4, 'text-anchor': 'end'}).textContent = `${(max * i / 4 / 1000).toFixed(1)} µs`;
  }
  groups.forEach((points, group) => {
    const coords = points.map(({record, value}) => [
      90 + runs.indexOf(`${record.run_id}/${record.run_attempt}`) * 680 / Math.max(1, runs.length - 1),
      250 - value / max * 220]);
    svg('polyline', {points: coords.map(p => p.join(',')).join(' '), fill: 'none', stroke: colors[group % colors.length], 'stroke-width': 2});
    points.forEach(({record, value}, index) => {
      const dot = svg('circle', {cx: coords[index][0], cy: coords[index][1], r: 5, fill: colors[group % colors.length]});
      const title = document.createElementNS('http://www.w3.org/2000/svg', 'title');
      title.textContent = `${record.commits.head.slice(0, 10)}: ${(value / 1000).toFixed(2)} µs`;
      dot.append(title);
      const row = table.insertRow();
      for (const text of [record.timestamp, record.commits.head.slice(0, 10), (value / 1000).toFixed(2),
        record.environment.rust.split('\n')[0], record.environment.cpu,
        `Series ${group + 1} · ${record.harness.slice(0, 8)} · ${record.environment.os} · ${record.environment.image}`]) row.insertCell().textContent = text;
      const cell = row.insertCell();
      for (const [label, file] of [['JSON', record.file], ['Raw', record.file.replace(/\.json$/, '.txt')]]) {
        const link = document.createElement('a');
        link.textContent = label;
        link.href = `data/${encodeURIComponent(file)}`;
        cell.append(link, ' ');
      }
    });
  });
  svg('text', {x: 400, y: 290, 'text-anchor': 'middle'}).textContent = 'CI runs → (ordered by run, not elapsed time)';
}

if (typeof document !== 'undefined') {
  const status = document.querySelector('#status');
  try {
    const response = await fetch('./data/index.json');
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const records = await response.json();
    const runner = document.querySelector('#runner');
    const operation = document.querySelector('#operation');
    for (const name of [...new Set(records.map(r => r.runner))]) runner.add(new Option(name, name));
    for (const name of operations(records)) operation.add(new Option(name, name));
    const update = () => draw(records, runner.value, operation.value, document);
    runner.addEventListener('change', update);
    operation.addEventListener('change', update);
    status.textContent = records.length ? `${records.length} recorded worker runs. Lower is faster.` : 'No measurements published yet. The first successful main benchmark run will populate this page.';
    update();
  } catch (error) {
    status.textContent = `Could not load performance history: ${error.message}`;
    status.setAttribute('role', 'alert');
  }
}
