
"""
vaultwares_agentciation — importlib shim over the vaultwares-agentciation submodule.

The canonical source files live in ../vaultwares-agentciation/ (hyphen), which is
the git submodule.  Because Python cannot import from a package directory whose
name contains a hyphen, this package (underscore) loads each module from the
submodule directory via importlib and registers it under the
``vaultwares_agentciation.*`` namespace so that all intra-package relative imports
inside the submodule files resolve correctly.

No code in the submodule needs to be modified.  Consumer code continues to use::

    from vaultwares_agentciation import ExtrovertAgent, LonelyManager, ...

If the submodule has not been initialised, run::

    git submodule update --init
"""

import importlib.util
import sys
from pathlib import Path

# The canonical source is the sibling directory produced by:
#   git submodule update --init
_SUBMODULE_DIR = (Path(__file__).parent.parent / "vaultwares-agentciation").resolve()

_PACKAGE = __name__  # "vaultwares_agentciation"

# Module load order must respect intra-package dependencies:
#   enums (no deps)
#   → redis_coordinator (no internal deps)
#   → agent_base (depends on redis_coordinator, enums)
#   → extrovert_agent (depends on agent_base, enums)
#   → lonely_manager (depends on extrovert_agent, enums)
_MODULES = [
    "enums",
    "redis_coordinator",
    "agent_base",
    "extrovert_agent",
    "lonely_manager",
]


def _load_submodule(name: str):
    full_name = f"{_PACKAGE}.{name}"
    if full_name in sys.modules:
        return sys.modules[full_name]
    path = _SUBMODULE_DIR / f"{name}.py"
    if not path.is_file():
        raise ImportError(
            f"Cannot load '{full_name}': '{path}' not found. "
            "The vaultwares-agentciation submodule may not be initialised — "
            "run `git submodule update --init` and try again."
        )
    spec = importlib.util.spec_from_file_location(full_name, path)
    if spec is None:
        raise ImportError(
            f"Cannot create module spec for '{full_name}' from '{path}'. "
            "Verify that the file is a valid Python source file."
        )
    module = importlib.util.module_from_spec(spec)
    # Register before exec so cyclic/relative imports resolve to this entry.
    sys.modules[full_name] = module
    module.__package__ = _PACKAGE
    spec.loader.exec_module(module)
    return module


for _name in _MODULES:
    _load_submodule(_name)

from .enums import AgentStatus
from .redis_coordinator import RedisCoordinator
from .agent_base import AgentBase
from .extrovert_agent import ExtrovertAgent
from .lonely_manager import LonelyManager

__all__ = [
    "AgentStatus",
    "RedisCoordinator",
    "AgentBase",
    "ExtrovertAgent",
    "LonelyManager",
]
