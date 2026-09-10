import json,glob,collections,bisect,pathlib,argparse
parser=argparse.ArgumentParser(description="Summarize diagnostic CDP allocation samples using a matching source map.")
parser.add_argument("sourcemap")
parser.add_argument("results", help="Extracted directory containing alloc-*.json")
parser.add_argument("output")
args=parser.parse_args()
m=json.load(open(args.sourcemap))
chars='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
def decode(s):
 values=[];v=shift=0
 for c in s:
  x=chars.index(c);v|=(x&31)<<shift
  if x&32:shift+=5
  else:values.append(-(v>>1) if v&1 else v>>1);v=shift=0
 return values
lines=[];source=line=col=name=0
for encoded in m['mappings'].split(';'):
 gen=0;segments=[]
 for part in encoded.split(','):
  if not part:continue
  v=decode(part);gen+=v[0]
  if len(v)>1:
   source+=v[1];line+=v[2];col+=v[3]
   if len(v)>4:name+=v[4]
   segments.append((gen,source,line,col))
 lines.append(segments)
def resolve(f):
 if not f['url'].endswith('/app.js'):return f['functionName'] or '(native)'
 seg=lines[f['lineNumber']];i=bisect.bisect_right([s[0] for s in seg],f['columnNumber'])-1
 if i<0:return str(f)
 _,src,ln,cl=seg[i];path=m['sources'][src].split('/volmap/')[-1]
 return f'{path}:{ln+1}:{cl+1} ({f["functionName"]})'
result={}
for p in glob.glob(str(pathlib.Path(args.results)/'alloc-*.json')):
 d=json.load(open(p))['profile'];groups=collections.Counter();attributed=collections.Counter()
 def visit(n,stack):
  f=n['callFrame'];s=stack+[resolve(f)];groups[' > '.join(s[-3:])]+=n['selfSize']
  # Attribute native builtins to their nearest named script caller.
  owners=[x for x in s if x.startswith('web/') or x in ['queryRole','querySelectorAll']]
  attributed[owners[-1] if owners else 'automation/native']+=n['selfSize']
  for c in n['children']:visit(c,s)
 visit(d['head'],[])
 result[pathlib.Path(p).name]={'totalSampledBytes':sum(groups.values()),'stacks':[{'bytes':v,'stack':k} for k,v in groups.most_common(30)],'attribution':[{'bytes':v,'source':k} for k,v in attributed.most_common(25)]}
 print(pathlib.Path(p).name)
 for k,v in attributed.most_common(12):print(round(v/1048576,3),k)
pathlib.Path(args.output).write_text(json.dumps(result,indent=2)+'\n')
