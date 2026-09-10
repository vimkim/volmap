"""Read native categories, preserving allocator paths rather than adding aliases."""
import pathlib,json,gzip,collections,sys
root=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else pathlib.Path(__file__).parent
out={}
for directory in sorted(root.iterdir()):
 if not directory.is_dir() or not (directory/'phases.json').exists():continue
 metadata=json.loads((directory/'phases.json').read_text());phases={}
 if metadata['name']=='firefox':
  for phase in metadata['phases']:
   path=directory/(phase['label']+'.json.gz')
   if not path.exists():continue
   data=json.load(gzip.open(path));counts=collections.Counter()
   for row in data['reports']:
    if row['units']!=0:continue
    p=row['path'];value=row['amount']
    if p.startswith('explicit/gfx/'):counts['gfx-total']+=value
    if p=='explicit/gfx/webrender/swgl':counts['swgl']+=value
    if p=='explicit/gfx/webrender/frame-allocator':counts['frame-allocator']+=value
    if p=='explicit/gfx/webrender/frame-allocator/render-tasks':counts['render-tasks']+=value
    if '/layout/' in p:counts['window-layout']+=value
    if '/dom/' in p:counts['window-dom']+=value
    if '/js-realm(' in p:counts['window-js']+=value
   phases[phase['label']]=dict(counts)
 else:
  data=json.load(gzip.open(directory/'trace.json.gz'));events=data['traceEvents']
  names={e['pid']:e['args']['name'] for e in events if e.get('name')=='process_name'}
  markers=sorted((e['ts'],e['args']['sync_id']) for e in events if e.get('name')=='clock_sync')
  if not markers:
   out[directory.name]={'excluded':'Unmarked initial trace; trace-local dump IDs differ from returned CDP GUIDs.'};continue
  for event in events:
   allocators=event.get('args',{}).get('dumps',{}).get('allocators',{})
   if not allocators:continue
   labels=[label for ts,label in markers if ts<=event['ts']]
   if not labels:continue
   counts=phases.setdefault(labels[-1],collections.Counter())
   for path,allocator in allocators.items():
    value=allocator.get('attrs',{}).get('size',{}).get('value')
    if value and path in {'cc/tile_memory','gpu/shared_images','shared_memory','blink_gc','v8','malloc','partition_alloc'}:
     counts[f'{names.get(event["pid"],event["pid"])}/{path}']+=int(value,16)
 out[directory.name]={'controlMode':metadata.get('controlMode','physical'),'phases':phases,'dom':{p['label']:p['dom'] for p in metadata['phases']}}
(root/'native-summary.json').write_text(json.dumps(out,indent=2)+'\n')
for name,data in out.items():
 if 'phases' not in data:continue
 print(name)
 for phase,counts in data['phases'].items():
  keys=['swgl','frame-allocator','gfx-total'] if name.startswith('firefox') else ['Renderer/cc/tile_memory','Renderer/blink_gc','Renderer/v8']
  print(' ',phase, {key:round(counts.get(key,0)/1048576,2) for key in keys})
