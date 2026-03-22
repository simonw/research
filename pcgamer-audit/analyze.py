#!/usr/bin/env python3
"""Analyze network capture JSON from rodney network-log command."""

import json
import sys
from collections import defaultdict, Counter
from urllib.parse import urlparse

def analyze(filename):
    with open(filename) as f:
        data = json.load(f)

    total_enc = sum(r['encoded_size'] for r in data)
    total_dec = sum(r['decoded_size'] for r in data)

    print(f"File: {filename}")
    print(f"Total requests: {len(data)}")
    print(f"Total transfer: {total_enc:,.0f} bytes ({total_enc/1024/1024:.1f} MB)")
    print(f"Total decoded:  {total_dec:,.0f} bytes ({total_dec/1024/1024:.1f} MB)")
    print()

    # By resource type
    by_type = defaultdict(lambda: {'count': 0, 'encoded': 0, 'decoded': 0})
    for r in data:
        t = r['resource_type']
        by_type[t]['count'] += 1
        by_type[t]['encoded'] += r['encoded_size']
        by_type[t]['decoded'] += r['decoded_size']

    print("=== BY RESOURCE TYPE ===")
    print(f"{'Type':<20} {'Count':>6} {'Transfer KB':>14} {'Decoded KB':>14}")
    for t, v in sorted(by_type.items(), key=lambda x: x[1]['encoded'], reverse=True):
        print(f"{t:<20} {v['count']:>6} {v['encoded']/1024:>14,.1f} {v['decoded']/1024:>14,.1f}")
    print()

    # By domain
    by_domain = defaultdict(lambda: {'count': 0, 'encoded': 0, 'decoded': 0})
    for r in data:
        try:
            domain = urlparse(r['url']).netloc
        except:
            domain = 'unknown'
        by_domain[domain]['count'] += 1
        by_domain[domain]['encoded'] += r['encoded_size']
        by_domain[domain]['decoded'] += r['decoded_size']

    print("=== BY DOMAIN (top 30) ===")
    print(f"{'Domain':<55} {'Count':>6} {'Transfer KB':>14}")
    for d, v in sorted(by_domain.items(), key=lambda x: x[1]['encoded'], reverse=True)[:30]:
        print(f"{d:<55} {v['count']:>6} {v['encoded']/1024:>14,.1f}")
    print()

    # Top requests
    print("=== TOP 20 LARGEST REQUESTS ===")
    sorted_data = sorted(data, key=lambda x: x['encoded_size'], reverse=True)
    for r in sorted_data[:20]:
        print(f"{r['encoded_size']/1024:>10,.1f} KB | {r['resource_type']:<12} | {r['url'][:90]}")
    print()

    # Ad-tech vs content
    adtech_keywords = [
        'doubleclick', 'googlesyndication', 'googletagmanager', 'facebook', 'tiktok',
        'analytics', 'doubleverify', 'permutive', 'pubmatic', 'adnxs', 'rubiconproject',
        'scorecardresearch', 'crwdcntrl', 'dotmetrics', 'brandmetrics', 'amazon-adsystem',
        'ml314', 'adtrafficquality', 'adnami', 'btloader', 'pbxai', 'studiostack',
        'privacy-mgmt', 'privacymanager', 'webcontentassessor', 'id5-sync', 'cpx.to',
        'amxrtb', '33across', 'euid.eu', 'p-n.io', 'hawky'
    ]
    first_party = ['pcgamer.com', 'futurecdn.net']

    ad_enc = sum(r['encoded_size'] for r in data
                 if any(k in urlparse(r['url']).netloc for k in adtech_keywords))
    fp_enc = sum(r['encoded_size'] for r in data
                 if any(k in urlparse(r['url']).netloc for k in first_party)
                 and not any(k in urlparse(r['url']).netloc for k in adtech_keywords))

    print("=== AD-TECH vs CONTENT ===")
    print(f"Ad-tech/tracking: {ad_enc/1024:,.1f} KB ({ad_enc/total_enc*100:.1f}%)")
    print(f"First-party:      {fp_enc/1024:,.1f} KB ({fp_enc/total_enc*100:.1f}%)")
    print(f"Other:            {(total_enc-ad_enc-fp_enc)/1024:,.1f} KB ({(total_enc-ad_enc-fp_enc)/total_enc*100:.1f}%)")


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: python analyze.py <network-log.json>")
        sys.exit(1)
    analyze(sys.argv[1])
