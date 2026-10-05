import sys,json
from pathlib import Path
sys.path.insert(0,'zpl-font-extract/scripts')
import ascii_font_metrics as m
import reconstruct_ascii_font as r
import joint_hint_program as hints
from ascii_font_probe import PRINTABLE,prepare_spacing
from capture_font_probe import now,sha
fixtures=Path('zpl-font-extract/tests/fixtures');root=fixtures/'ascii-spacing-followup-20261004';root.mkdir()
source=fixtures/'ascii-reconstruction-20261004';data=(source/'candidates.json').read_bytes();old=json.loads(data)['targeted']
pages,provenance,digest=r.load(fixtures/'ascii-font-20261004',{'spacing'})
advances,report=m.fit(m.extract(pages),representative='midpoint')
new=json.loads(json.dumps(old));new['advances']=advances
states={'weighted':old,'midpoint':new}
r.save(root/'candidates.json',states)
for label,state in states.items():(root/(label+'.ttf')).write_bytes(hints.build(state))
sizes=(23,53,101,157)
# Verify new nominal configurations have not appeared in either existing spacing partition.
previous=json.load(open(fixtures/'ascii-font-20261004/plan.json'))
for c in previous['campaigns']:
 if c['role']=='spacing':
  manifest=json.load(open(fixtures/'ascii-font-20261004'/c['name']/'manifest.json'))
  assert not (set(sizes)&set(manifest['sizes']))
captures=[]
for size in sizes:
 name=f'validation-spacing-{size}';prepare_spacing(root/name,PRINTABLE,[size],'validation')
 captures.append(dict(name=name,role='spacing',manifest_sha256=sha((root/name/'manifest.json').read_bytes())))
r.save(root/'fit.json',dict(training_capture_provenance=provenance,training_sampling_sha256=digest,
                          source_candidates_sha256=sha(data),representative='midpoint',report=report,
                          changed_glyphs=sum(old['advances'][c]!=advances[c] for c in PRINTABLE)))
r.save(root/'audit-plan.json',dict(schema='ascii-spacing-audit-v1',frozen_at=now(),
      printer=provenance[0]['printer'],engine_sha256=json.load(open(source/'plan.json'))['engine_sha256'],
      candidates_sha256=sha((root/'candidates.json').read_bytes()),fit_sha256=sha((root/'fit.json').read_bytes()),
      captures=captures,fonts=[dict(label=k,file=k+'.ttf',sha256=sha((root/(k+'.ttf')).read_bytes())) for k in states],
      role='new spacing validation after the original full-ASCII audit exposed interval-selection bias'))
print('changed advances',sum(old['advances'][c]!=advances[c] for c in PRINTABLE),'space',old['advances'][' '],advances[' '])
