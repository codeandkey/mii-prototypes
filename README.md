# lmc
lightweight module cache

---

lmc automatically caches and loads environment modules for you. It works by hooking into your shells' "command not found" event and using that opportunity to see if any modules
would provide your command.

Normal shell:
~~~
[molecuul@pine ~]$ mcl
bash: mcl: command not found
[molecuul@pine ~]$ module load mcl
[molecuul@pine ~]$ mcl
[mcl] usage: mcl <-|file name> [options], do 'mcl -h' or 'man mcl' for help
~~~

lmc-enabled shell:
~~~
[molecuul@pine ~]$ mcl
[lmc] autoloading mcl/14-137-4aumkvp..
[mcl] usage: mcl <-|file name> [options], do 'mcl -h' or 'man mcl' for help
~~~

lmc will prompt you if multiple modules can provide your command:
~~~
[molecuul@pine ~]$ ace2sam
[lmc] select a module to load:
    1) samtools/1.8-r54nmop
    2) samtools/1.6-lyscjka
    3) samtools/1.7-kglvk7q
[lmc] enter a selection (1-3, q to abort) [1]: 2
[lmc] loading samtools/1.6-lyscjka..
Usage:   ace2sam [-pc] <in.ace>
~~~

### features

- Streamlined module environment
- Blazing speed (caches ~70,000 binary entries / second on my machine)
- Lightweight source, few dependencies

### installation

- Clone the repository: `git clone https://github.com/molecuul/lmc a~/.lmc"`
- Build the source: `cd ~/.lmc && make`
- `bash` users: `echo "source ~/.lmc/usr/share/lmc/init/bash" >> ~/.bashrc`
- `zsh` users: `echo "source ~/.lmc/usr/share/lmc/init/zsh" >> ~/.zshrc`
