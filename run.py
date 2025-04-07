import json
from urllib.parse import urlparse
with open("bbb.json") as f:
    s = json.load(f)

count = {}
for v in s:
    k = urlparse(v["file"]).path
    count[k] = count.get(k, 0) + len(v["cases"])
    if len(v["cases"]) == 0:
        count[k] += 1

with open("out.json", "w") as f:
    json.dump(count, f)
