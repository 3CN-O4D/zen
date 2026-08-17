#!/usr/bin/env bash
# Bash completion for the Zen language runtime
# Install: source this file, or place in /etc/bash_completion.d/zen
#   source editors/bash/zen-completion.bash

_zen() {
    local cur prev words cword
    _init_completion || return

    local commands="run check lint repl pm help version"
    local pm_subcommands="init install list freeze remove info verify pack publish"
    local subcommands=""

    case "${prev}" in
        zen)
            case "${cur}" in
                -*)  COMPREPLY=($(compgen -W "--version --help -e --eval" -- "${cur}")) ;;
                *)   COMPREPLY=($(compgen -W "${commands}" -- "${cur}")) ;;
            esac
            return
            ;;
        run|check|lint)
            _filedir '@(z)'
            return
            ;;
        pm)
            COMPREPLY=($(compgen -W "${pm_subcommands}" -- "${cur}"))
            return
            ;;
        install)
            case "${words[$((cword-2))]}" in
                install)
                    COMPREPLY=($(compgen -W "--force -r --requirements" -- "${cur}"))
                    return
                    ;;
            esac
            ;;
        init)
            COMPREPLY=($(compgen -W "--name --desc" -- "${cur}"))
            return
            ;;
        remove|uninstall|info|verify)
            return
            ;;
        pack)
            _filedirs
            return
            ;;
        publish)
            if [ "$cword" -eq 3 ]; then
                _filedirs
            else
                # Git remote names
                local remotes
                remotes=$(git remote 2>/dev/null)
                COMPREPLY=($(compgen -W "${remotes}" -- "${cur}"))
            fi
            return
            ;;
        -e|--eval)
            return
            ;;
    esac

    # Default: complete .z files
    _filedir '@(z)'
}

complete -F _zen zen
