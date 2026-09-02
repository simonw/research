import sys
data=open(sys.argv[1],'rb').read()
pos=8
def leb(p):
    r=0;s=0
    while True:
        b=data[p];p+=1;r|=(b&0x7f)<<s;s+=7
        if not b&0x80: return r,p
def name(p):
    n,p=leb(p);return data[p:p+n].decode(),p+n
while pos<len(data):
    sid=data[pos];pos+=1
    size,pos=leb(pos)
    end=pos+size
    if sid==2:
        n,p=leb(pos)
        for i in range(n):
            m,p=name(p);f,p=name(p);kind=data[p];p+=1
            if kind==0: idx,p=leb(p); print('import func',m,f,'type',idx)
            elif kind==3: p+=2; print('import global',m,f,'ty',data[p-2])
            else: print('import other',m,f,kind); break
    if sid==7:
        n,p=leb(pos)
        for i in range(n):
            f,p=name(p);kind=data[p];p+=1;idx,p=leb(p);print('export',f,kind,idx)
    pos=end
