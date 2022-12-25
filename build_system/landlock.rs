use landlock::{
    path_beneath_rules, Access, AccessFs, Compatible, PathFd, Ruleset, RulesetAttr, RulesetCreated,
    RulesetCreatedAttr, RulesetError, ABI,
};

/// Base landlock ruleset
///
/// This allows access to various essential system locations.
pub(super) fn base_ruleset() -> RulesetCreated {
    let abi = ABI::V2;
    let access_all = AccessFs::from_all(abi);
    let access_read = AccessFs::from_read(abi);
    Ruleset::new()
        .set_best_effort(false)
        .handle_access(access_all)
        .unwrap()
        .create()
        .unwrap()
        .add_rules(path_beneath_rules(&["/"], access_read))
        .unwrap()
        .add_rules(path_beneath_rules(&["/tmp", "/dev/null"], access_all))
        .unwrap()
}

pub(super) fn lock_fetch() {
    let abi = landlock::ABI::V2;
    let access_all = landlock::AccessFs::from_all(abi);
    let access_read = landlock::AccessFs::from_read(abi);
    base_ruleset()
        .add_rules(landlock::path_beneath_rules(
            &[
                std::env::current_dir().unwrap().join("build"), // FIXME only enable during ./y.rs build
            ],
            access_all,
        ))
        .unwrap()
        .add_rules(path_beneath_rules(
            [
                "/home/bjorn/.cargo/".to_owned().into(),
                std::env::current_dir().unwrap().join("download"),
                std::env::current_dir().unwrap().join("build_sysroot"), // FIXME move to download/
            ],
            access_all,
        ))
        .unwrap()
        .restrict_self()
        .unwrap();
}

pub(super) fn lock_build() {
    let abi = landlock::ABI::V2;
    let access_all = landlock::AccessFs::from_all(abi);
    let access_read = landlock::AccessFs::from_read(abi);
    base_ruleset()
        .add_rules(landlock::path_beneath_rules(
            &[
                std::env::current_dir().unwrap().join("build"),
                std::env::current_dir().unwrap().join("dist"),
            ],
            access_all,
        ))
        .unwrap()
        .restrict_self()
        .unwrap();
}
