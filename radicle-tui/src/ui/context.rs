use radicle::prelude::{Id, Project, Signer};
use radicle::Profile;

use radicle::storage::git::Repository;
use radicle::storage::ReadStorage;

pub struct Context {
    profile: Profile,
    id: Id,
    project: Project,
    repository: Repository,
    signer: Box<dyn Signer>,
}

impl Context {
    pub fn new(profile: Profile, id: Id, project: Project, signer: Box<dyn Signer>) -> Self {
        let repository = profile.storage.repository(id).unwrap();
        Self {
            id,
            profile,
            project,
            repository,
            signer
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    pub fn signer(&self) -> &Box<dyn Signer> {
        &self.signer
    }
}
