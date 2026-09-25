//! Checks the cached paths CMake will reuse before it can report a useful error.

use std::path::Path;

pub(crate) fn stale_cache_reason(cache: &str, engine: &Path) -> Option<String> {
    for line in cache.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "CMAKE_OSX_SYSROOT:STRING" | "CMAKE_OSX_SYSROOT:PATH"
                if missing_absolute_path(value) =>
            {
                return Some(format!("macOS SDK `{value}` no longer exists"));
            }
            "CMAKE_COMMAND:INTERNAL" if missing_absolute_path(value) => {
                return Some(format!("CMake executable `{value}` no longer exists"));
            }
            "CMAKE_HOME_DIRECTORY:INTERNAL" if Path::new(value) != engine => {
                return Some(format!(
                    "source directory `{value}` differs from `{}`",
                    engine.display()
                ));
            }
            _ => {}
        }
    }
    None
}

fn missing_absolute_path(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute() && !path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_sdk_has_an_actionable_reason() {
        let cache = "CMAKE_OSX_SYSROOT:STRING=/missing/MacOSX26.0.sdk\n";
        let reason = stale_cache_reason(cache, Path::new("/engine")).unwrap();
        assert!(reason.contains("MacOSX26.0.sdk"));
    }

    #[test]
    fn cache_from_another_worktree_is_rejected() {
        let cache = "CMAKE_HOME_DIRECTORY:INTERNAL=/old/worktree/vendor/CyberRemesherAndUV\n";
        let reason = stale_cache_reason(cache, Path::new("/new/vendor/CyberRemesherAndUV"))
            .expect("stale source directory");
        assert!(reason.contains("source directory"));
    }

    #[test]
    fn current_cache_paths_are_accepted() {
        let engine = std::env::current_dir().unwrap();
        let cache = format!("CMAKE_HOME_DIRECTORY:INTERNAL={}\n", engine.display());
        assert_eq!(stale_cache_reason(&cache, &engine), None);
    }
}
