---
author: Steve Sosik
date: 2021-08-10T09:17:23-0400
tags:
- docker
- cli
- xq
- tika
- xapiary
- rust
title: Build mdq in a docker container
---

```bash
# Start bash in the container
sudo docker run -ti ubuntu /bin/bash

# fetch dependencies, source, and build it
apt-get update  --yes
apt-get upgrade  --yes
apt-get install  --yes aptitude git make tar xz-utils vim g++ curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
git clone --recurse-submodules https://github.com/ssosik/mdq.git
cd mdq
make
```

In a rust container:
```bash
docker run -ti rust /bin/bash

# fetch dependencies, source, and build it
apt-get update  --yes && \
  apt-get upgrade  --yes && \
  apt-get install  --yes git xz-utils vim build-essential && \
  git clone --recurse-submodules https://github.com/ssosik/mdq.git && \
  cd mdq && \
  make
```
