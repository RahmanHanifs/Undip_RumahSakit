#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Patient {
    id: u64,
    name: String,
    age: u32,
    gender: String,
    room_number: Option<u32>,
    admitted_at: u64,
}

impl Storable for Patient {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Patient {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Patient, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct PatientPayload {
    name: String,
    age: u32,
    gender: String,
}

#[ic_cdk::query]
fn get_patient(id: u64) -> Result<Patient, Error> {
    match _get_patient(&id) {
        Some(patient) => Ok(patient),
        None => Err(Error::NotFound {
            msg: format!("a patient with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_patient(patient: PatientPayload) -> Option<Patient> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    let patient = Patient {
        id,
        name: patient.name,
        age: patient.age,
        gender: patient.gender,
        room_number: None,
        admitted_at: time(),
    };
    do_insert(&patient);
    Some(patient)
}

#[ic_cdk::update]
fn update_patient(id: u64, payload: PatientPayload) -> Result<Patient, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut patient) => {
            patient.name = payload.name;
            patient.age = payload.age;
            patient.gender = payload.gender;
            do_insert(&patient);
            Ok(patient)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update a patient with id={}. patient not found",
                id
            ),
        }),
    }
}

fn do_insert(patient: &Patient) {
    STORAGE.with(|service| service.borrow_mut().insert(patient.id,patient.clone()));
}

#[ic_cdk::update]
fn assign_room(id: u64, room_number: u32) -> Result<Patient, Error> {
    match STORAGE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut patient) => {
            patient.room_number = Some(room_number);
            do_insert(&patient);
            Ok(patient)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't assign a room to a patient with id={}. patient not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn discharge_patient(id: u64) -> Result<Patient, Error> {
    match STORAGE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut patient) => {
            patient.room_number = None;
            do_insert(&patient);
            Ok(patient)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't discharge a patient with id={}. patient not found",
                id
            ),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

fn _get_patient(id: &u64) -> Option<Patient> {
    STORAGE.with(|service| service.borrow().get(id))
}

ic_cdk::export_candid!();