#!/usr/bin/env python3
import json, os, re, time, traceback, subprocess, statistics, shlex
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Any, List

BASE = Path('/home/kahtaf/Documents/workspace/research/browser-automation-benchmark')
ART = BASE / 'artifacts'
RES = BASE / 'results'
ART.mkdir(parents=True, exist_ok=True)
RES.mkdir(parents=True, exist_ok=True)

URLS = {
    'x': {'page_type':'post','url':'https://x.com/jack/status/20', 'expected':['post_text','author_handle','timestamp','canonical_url']},
    'reddit': {'page_type':'post','url':'https://www.reddit.com/r/Python/comments/10wxbk8/whats_everyone_working_on_this_week/', 'expected':['post_title','post_body','subreddit','author','timestamp','canonical_url']},
    'linkedin': {'page_type':'company','url':'https://www.linkedin.com/company/microsoft/', 'expected':['title_or_company','location','page_url','key_metadata']},
    'instagram': {'page_type':'profile','url':'https://www.instagram.com/instagram/', 'expected':['username','timestamp','canonical_url']},
}

COOKIES_RAW = {
    'x': '''.x.com\tTRUE\t/\tTRUE\t1804045794\tnight_mode\t2\n.x.com\tTRUE\t/\tFALSE\t1807066194\t__cuid\t275a9d3ceb3640b1995368556e0e8f1b\nx.com\tFALSE\t/\tFALSE\t0\tlang\ten\n.x.com\tTRUE\t/\tTRUE\t1804913448\tpersonalization_id\t\"v1_lI2PyebYalOAhBfGH3Rsfw==\"\n.x.com\tTRUE\t/\tTRUE\t1806505040\tdnt\t1\n.x.com\tTRUE\t/\tTRUE\t1806505040\tkdt\tSiZbooJQPqWiC4mYFLvUcqv47oC8kbZXCSPWlYTK\n.x.com\tTRUE\t/\tTRUE\t1807069795\tauth_multi\t\"1677220568:d10163a806db01f8b460378cefc72968ed0ef76d\"\n.x.com\tTRUE\t/\tTRUE\t1806505040\tauth_token\tb905351bfaff2001231b9ce43e16b9a020ea4a7d\n.x.com\tTRUE\t/\tTRUE\t1807069800\tguest_id_ads\tv1%3A177194504045419875\n.x.com\tTRUE\t/\tTRUE\t1807069800\tguest_id_marketing\tv1%3A177194504045419875\n.x.com\tTRUE\t/\tTRUE\t1806505041\tguest_id\tv1%3A177194504045419875\n.x.com\tTRUE\t/\tTRUE\t1804045800\ttwid\tu%3D1481080264730288129\n.x.com\tTRUE\t/\tTRUE\t1806505041\tct0\t2e1df0c00c40f84751c8193f43e9f97f441b98c588f1550b756237542325325bb951d2a74947c7b32d0abb66d3fd7b1988c4d8c281f4933da2d7592245e8a47c351e1a4f1dcd749e11293a52ddc2e4c8\n''',
    'linkedin': '''.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tbcookie\t\"v=2&9765043f-0558-4286-8aa3-de7c49100928\"\n.www.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tbscookie\t\"v=1&20241223214557c8a2393d-1dad-4ebe-86b8-03f258783920AQFmLjr_--OxwvWkz-67Rt9fYk65vXmc\"\n.www.linkedin.com\tTRUE\t/\tTRUE\t1791581959\tli_rm\tAQEk5nVIAGbLrAAAAZP1fEsLj4fYYeuIJfZqzz2SLtQyO82Do_9PCerZNOjv7yC4LSYNxqZ5ccFk7lveh6mcr6TfZvmbpW5vBXTgloeIuE9pJQjNLAs6RBQXhUjUVBq6H5j3rZa6ZWVNKxjJrvWBTU5EfGGJeVWJ42YgMcHXVJK1CvrZV7Und0F5MDt-mPc1ZuJxmvBRUau4TJsZh80QzmYqsyB_F_xYxb1sdkJDe1J8_3kyh0mwayaWD-0-6sXZf4uBlzKs9ii--OLLxZ1C7vLT9wFHGuhGS9O_ZFKO5HOWc3tYMeB52omoXsQg6sBDi2g14fnFtjKQVSjUX3I\n.www.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tJSESSIONID\t\"ajax:1957846287504080487\"\n.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tliap\ttrue\n.www.linkedin.com\tTRUE\t/\tTRUE\t1803697695\tli_at\tAQEDARDtRBUEyys_AAABmcrqBeMAAAGcwR3_6E0AnmjJXw7xTRcC33KHAxzBp7OfsN6zEjMPPRLW8akBqTJHqN1KXtRku1NIBVtnRgpfDUsBZorj2lAJxi4fynv3-tjGEP0FZy_7feEwTNDkmS-uueub\n''',
    'instagram': '''.instagram.com\tTRUE\t/\tTRUE\t1774480946\tdatr\tMha1Z9A5p_QlLjwHV3MLECiA\n.instagram.com\tTRUE\t/\tTRUE\t1774480947\tmid\tZ7UWMgAEAAFmoKDVkfGZj_YuoYV1\n.instagram.com\tTRUE\t/\tTRUE\t1803526655\tig_did\t09CD82BC-DDF7-4565-9C65-6283461A74DB\n.instagram.com\tTRUE\t/\tTRUE\t1780285828\tds_user_id\t79028106685\n.instagram.com\tTRUE\t/\tTRUE\t1807069828\tcsrftoken\tCqBXIT8gwzwFYZXxjW3F26bHeJhibSfN\n.instagram.com\tTRUE\t/\tTRUE\t1804045828\tsessionid\t79028106685%3AoIAFrJKUG0TRYj%3A13%3AAYj8V_hmLbkJ84ckqo-XzOAYtFUxABhCoiNymghShg\n'''
}


def parse_cookies(site):
    out=[]
    raw=COOKIES_RAW.get(site,'').strip()
    if not raw: return out
    for ln in raw.splitlines():
        p=ln.split('\t')
        if len(p)<7: continue
        domain, include_sub, path, secure, exp, name, value = p[:7]
        ck={'name':name,'value':value.strip('"'),'domain':domain,'path':path,'secure':secure=='TRUE'}
        if exp.isdigit() and int(exp)>0: ck['expires']=int(exp)
        out.append(ck)
    return out

BLOCK_PAT = re.compile(r'captcha|challenge|suspicious|unusual traffic|verify|log in|sign in|access denied|temporarily unavailable', re.I)

def classify(text, extracted, expected):
    t=(text or '')[:30000]
    if BLOCK_PAT.search(t):
        return 'blocked/challenged'
    found=sum(1 for k in expected if extracted.get(k))
    if found==len(expected) and found>0:
        return 'success'
    if found>0:
        return 'partial'
    return 'timeout'


def extract(site, text, url):
    t=text or ''
    e={}
    if site=='x':
        e['post_text']=next(iter(re.findall(r'"text":"([^"]{10,280})"', t)), '') or next(iter(re.findall(r'\n(.{20,280})\n.*?@', t)), '')
        e['author_handle']=next(iter(re.findall(r'@([A-Za-z0-9_]{1,15})', t)), '')
        e['timestamp']=next(iter(re.findall(r'\d{1,2}:\d{2}\s?(?:AM|PM)?.*?\d{4}|\d{4}-\d{2}-\d{2}T[^\s"]+', t)), '')
        e['canonical_url']=url if 'x.com' in (url or '') else ''
    elif site=='reddit':
        e['post_title']=next(iter(re.findall(r'<title>(.*?)</title>', t, re.I|re.S)), '').strip()
        e['post_body']=next(iter(re.findall(r'<shreddit-post[\s\S]*?>', t, re.I)), '')[:300]
        e['subreddit']=next(iter(re.findall(r'r/([A-Za-z0-9_]+)', t)), '')
        e['author']=next(iter(re.findall(r'u/([A-Za-z0-9_\-]+)', t)), '')
        e['timestamp']=next(iter(re.findall(r'\d+\s+(?:hours?|days?|minutes?)\s+ago|\d{4}-\d{2}-\d{2}T[^\s"]+', t)), '')
        e['canonical_url']=url if 'reddit.com' in (url or '') else ''
    elif site=='linkedin':
        e['title_or_company']=next(iter(re.findall(r'Microsoft|Company|LinkedIn', t, re.I)), '')
        e['location']=next(iter(re.findall(r'Redmond|United States|Toronto|Remote', t, re.I)), '')
        e['page_url']=url if 'linkedin.com' in (url or '') else ''
        e['key_metadata']=next(iter(re.findall(r'\d+[+,]?\s+employees|Information Technology', t, re.I)), '')
    elif site=='instagram':
        e['username']=next(iter(re.findall(r'instagram', t, re.I)), '')
        e['timestamp']=next(iter(re.findall(r'\d{4}-\d{2}-\d{2}T[^\s"]+', t)), '')
        e['canonical_url']=url if 'instagram.com' in (url or '') else ''
    return e


def run_agent_browser(site, cfg, attempt, cold):
    run_id=f"agent-browser_{site}_{attempt}_{'cold' if cold else 'warm'}"
    adir=ART/run_id; adir.mkdir(parents=True, exist_ok=True)
    session=f"ab-{site}-{'cold-'+str(attempt) if cold else 'warm'}"
    t0=time.time(); outcome='crash/error'; err=''; txt=''; final_url=''
    try:
        # open + wait + snapshot + screenshot
        cmds=[
            f"agent-browser --session {shlex.quote(session)} open {shlex.quote(cfg['url'])}",
            f"agent-browser --session {shlex.quote(session)} wait 6000",
            f"agent-browser --session {shlex.quote(session)} snapshot > {shlex.quote(str(adir/'snapshot.txt'))}",
            f"agent-browser --session {shlex.quote(session)} get html > {shlex.quote(str(adir/'page.html'))}",
            f"agent-browser --session {shlex.quote(session)} get url > {shlex.quote(str(adir/'url.txt'))}",
            f"agent-browser --session {shlex.quote(session)} screenshot {shlex.quote(str(adir/'screen.png'))}"
        ]
        cp=subprocess.run(' && '.join(cmds), shell=True, capture_output=True, text=True, timeout=70)
        (adir/'stdout.log').write_text(cp.stdout)
        (adir/'stderr.log').write_text(cp.stderr)
        if cp.returncode!=0:
            raise RuntimeError(f'agent-browser exit {cp.returncode}')
        txt=(adir/'page.html').read_text(errors='ignore') if (adir/'page.html').exists() else ''
        final_url=(adir/'url.txt').read_text(errors='ignore').strip() if (adir/'url.txt').exists() else ''
        ex=extract(site, txt, final_url)
        outcome=classify(txt, ex, cfg['expected'])
    except subprocess.TimeoutExpired:
        outcome='timeout'; err='timeout'
    except Exception as e:
        outcome='crash/error'; err=str(e)
    dt=time.time()-t0
    ex=extract(site, txt, final_url)
    rec={'tool':'agent-browser','site':site,'page_type':cfg['page_type'],'attempt':attempt,'cold':cold,'ts':datetime.now(timezone.utc).isoformat(),'run_id':run_id,'duration_s':round(dt,3),'outcome':outcome,'extracted':ex,'expected':cfg['expected'],'error':err,'artifact_dir':str(adir)}
    (adir/'record.json').write_text(json.dumps(rec,indent=2))
    return rec


def run_camoufox(site, cfg, attempt, cold):
    run_id=f"camofox-browser_{site}_{attempt}_{'cold' if cold else 'warm'}"
    adir=ART/run_id; adir.mkdir(parents=True, exist_ok=True)
    t0=time.time(); outcome='crash/error'; err=''; txt=''; final_url=''
    profile=BASE/f".profiles/camo-{site}-{'cold-'+str(attempt) if cold else 'warm'}"
    profile.mkdir(parents=True, exist_ok=True)
    try:
        script=f'''from playwright.sync_api import sync_playwright\nimport json\nurl={cfg['url']!r}\nprofile={str(profile)!r}\ncookies={parse_cookies(site)!r}\nout={str(adir)!r}\nwith sync_playwright() as p:\n  browser=p.chromium.launch_persistent_context(user_data_dir=profile, executable_path='/home/kahtaf/.cache/camoufox/camoufox-bin', headless=True)\n  page=browser.new_page()\n  if cookies:\n    browser.add_cookies(cookies)\n  page.goto(url, wait_until='domcontentloaded', timeout=45000)\n  page.wait_for_timeout(6000)\n  page.screenshot(path=f"{{out}}/screen.png", full_page=True)\n  html=page.content()\n  open(f"{{out}}/page.html",'w').write(html)\n  open(f"{{out}}/url.txt",'w').write(page.url)\n  open(f"{{out}}/title.txt",'w').write(page.title())\n  browser.close()\n'''
        cp=subprocess.run(['python3','-c',script],capture_output=True,text=True,timeout=85)
        (adir/'stdout.log').write_text(cp.stdout)
        (adir/'stderr.log').write_text(cp.stderr)
        if cp.returncode!=0:
            raise RuntimeError(f'camofox exit {cp.returncode}')
        txt=(adir/'page.html').read_text(errors='ignore') if (adir/'page.html').exists() else ''
        final_url=(adir/'url.txt').read_text(errors='ignore').strip() if (adir/'url.txt').exists() else ''
        ex=extract(site, txt, final_url)
        outcome=classify(txt, ex, cfg['expected'])
    except subprocess.TimeoutExpired:
        outcome='timeout'; err='timeout'
    except Exception as e:
        outcome='crash/error'; err=str(e)
    dt=time.time()-t0
    ex=extract(site, txt, final_url)
    rec={'tool':'camofox-browser','site':site,'page_type':cfg['page_type'],'attempt':attempt,'cold':cold,'ts':datetime.now(timezone.utc).isoformat(),'run_id':run_id,'duration_s':round(dt,3),'outcome':outcome,'extracted':ex,'expected':cfg['expected'],'error':err,'artifact_dir':str(adir)}
    (adir/'record.json').write_text(json.dumps(rec,indent=2))
    return rec


def run_scrapling(site, cfg, attempt, cold):
    run_id=f"scrapling_{site}_{attempt}_{'cold' if cold else 'warm'}"
    adir=ART/run_id; adir.mkdir(parents=True, exist_ok=True)
    t0=time.time(); outcome='crash/error'; err=''; txt=''; final_url=''
    profile=BASE/f".profiles/scrap-{site}-{'cold-'+str(attempt) if cold else 'warm'}"
    profile.mkdir(parents=True, exist_ok=True)
    try:
        script=f'''from scrapling.fetchers import StealthySession\nimport json\nurl={cfg['url']!r}\nprofile={str(profile)!r}\nout={str(adir)!r}\nwith StealthySession(user_data_dir=profile) as s:\n  r=s.fetch(url, headless=True, timeout=45000, wait=3000)\n  open(f"{{out}}/page.html",'w').write(r.text)\n  open(f"{{out}}/url.txt",'w').write(getattr(r,'url',url))\n'''
        cp=subprocess.run(['python3','-c',script],capture_output=True,text=True,timeout=90)
        (adir/'stdout.log').write_text(cp.stdout)
        (adir/'stderr.log').write_text(cp.stderr)
        if cp.returncode!=0:
            raise RuntimeError(f'scrapling exit {cp.returncode}')
        txt=(adir/'page.html').read_text(errors='ignore') if (adir/'page.html').exists() else ''
        final_url=(adir/'url.txt').read_text(errors='ignore').strip() if (adir/'url.txt').exists() else ''
        ex=extract(site, txt, final_url)
        outcome=classify(txt, ex, cfg['expected'])
    except subprocess.TimeoutExpired:
        outcome='timeout'; err='timeout'
    except Exception as e:
        outcome='crash/error'; err=str(e)
    dt=time.time()-t0
    ex=extract(site, txt, final_url)
    rec={'tool':'Scrapling','site':site,'page_type':cfg['page_type'],'attempt':attempt,'cold':cold,'ts':datetime.now(timezone.utc).isoformat(),'run_id':run_id,'duration_s':round(dt,3),'outcome':outcome,'extracted':ex,'expected':cfg['expected'],'error':err,'artifact_dir':str(adir)}
    (adir/'record.json').write_text(json.dumps(rec,indent=2))
    return rec


def summarize(records):
    by={}
    for r in records:
        k=(r['tool'],r['site'])
        by.setdefault(k,[]).append(r)
    summary=[]
    for k,rows in by.items():
        succ=[x for x in rows if x['outcome']=='success']
        partial=[x for x in rows if x['outcome']=='partial']
        blocked=[x for x in rows if x['outcome']=='blocked/challenged']
        timeout=[x for x in rows if x['outcome']=='timeout']
        crash=[x for x in rows if x['outcome']=='crash/error']
        succ_times=[x['duration_s'] for x in succ]
        p95=(statistics.quantiles(succ_times,n=20)[-1] if len(succ_times)>=2 else (succ_times[0] if succ_times else None))
        comp=[]
        corr=[]
        for x in rows:
            exp=len(x['expected'])
            got=sum(1 for f in x['expected'] if x['extracted'].get(f))
            comp.append(100*got/exp if exp else 0)
            corr.append(100*got/exp if exp else 0)
        stability=max(0,100-(len(timeout)+len(crash))*8)
        summary.append({
            'tool':k[0],'site':k[1],'attempts':len(rows),'success':len(succ),'partial':len(partial),'blocked':len(blocked),'timeout':len(timeout),'crash':len(crash),
            'success_rate_pct':round(100*len(succ)/len(rows),2),'block_rate_pct':round(100*len(blocked)/len(rows),2),'partial_rate_pct':round(100*len(partial)/len(rows),2),
            'avg_success_time_s':round(sum(succ_times)/len(succ_times),3) if succ_times else None,'p95_success_time_s':round(p95,3) if p95 else None,
            'data_completeness_pct':round(sum(comp)/len(comp),2),'correctness_spotcheck_pct':round(sum(corr)/len(corr),2),'stability_score':round(stability,2)
        })
    return summary


def main():
    records=[]
    for tool in ['agent-browser','camofox-browser','Scrapling']:
        for site,cfg in URLS.items():
            fn={'agent-browser':run_agent_browser,'camofox-browser':run_camoufox,'Scrapling':run_scrapling}[tool]
            records.append(fn(site,cfg,1,True))
            for i in range(1,11):
                records.append(fn(site,cfg,i,False))
            print('done', tool, site, flush=True)
    (RES/'attempts.json').write_text(json.dumps(records,indent=2))
    summary=summarize(records)
    (RES/'summary.json').write_text(json.dumps(summary,indent=2))
    print('completed benchmark, attempts=',len(records))

if __name__=='__main__':
    main()
