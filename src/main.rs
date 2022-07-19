#![allow(dead_code)]
// things:
// a total of 10 planes want to land
// 1 runway
// 3 hangars

// timings:
// a plane comes in every 3 seconds
// plane lands and takes off in 1 second
// plane rests for 2 seconds in hangar

// rules:
// if no hangars available, the plane is not allowed to land
// plane wants to take off after resting in hangar

// metrics:
// time taken to service all planes
// qty of planes accepted
// qty of planes denied
// average time of service (from land to takeoff)

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::sync::Semaphore;

#[derive(Debug)]
struct Plane {
    time_to_land: Duration,
    time_to_rest: Duration,
    created_at: Instant,
}

impl Plane {
    fn new() -> Self {
        Self {
            time_to_land: Duration::from_secs(1),
            time_to_rest: Duration::from_secs(2),
            created_at: Instant::now(),
        }
    }
}

impl Default for Plane {
    fn default() -> Self {
        Self::new()
    }
}

async fn plane_generator(number_of_planes: u8, interval: Duration, sender: Sender<Plane>) {
    let mut interval = tokio::time::interval(interval);
    for i in 0..number_of_planes {
        interval.tick().await;
        println!("sending plane: {i}");
        if sender.try_send(Plane::default()).is_err() {
            println!("plane is gone!");
            continue;
        }
    }
}

async fn plane_receiver(
    available_runways: Arc<Semaphore>,
    available_hangars: Arc<Semaphore>,
    plane: Plane,
    done_sender: Sender<Plane>,
) {
    let runway_permit = available_runways.try_acquire();
    if runway_permit.is_err() {
        println!("no runway available!");
        return;
    }

    let hangar_permit = available_hangars.try_acquire();
    if hangar_permit.is_err() {
        println!("no hangars left!");
        return;
    }

    tokio::time::sleep(plane.time_to_land).await;

    drop(runway_permit);

    tokio::time::sleep(plane.time_to_rest).await;

    let runway_permit = available_runways.acquire().await.unwrap();

    drop(hangar_permit);

    tokio::time::sleep(plane.time_to_land).await;

    drop(runway_permit);

    done_sender.send(plane).await.unwrap();
}

#[tokio::main]
async fn main() {
    let total_planes = 10;

    let (plane_tx, mut plane_rx) = tokio::sync::mpsc::channel::<Plane>(1);
    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<Plane>(10);

    let available_runways = Arc::new(Semaphore::new(1));
    let available_hangars = Arc::new(Semaphore::new(3));

    tokio::spawn(plane_generator(
        total_planes,
        Duration::from_secs(1),
        plane_tx,
    ));

    tokio::spawn(async move {
        while let Some(plane) = plane_rx.recv().await {
            tokio::spawn(plane_receiver(available_runways.clone(), available_hangars.clone(), plane, done_tx.clone()));
        }
    });

    while let Some(plane) = done_rx.recv().await {
        println!("received a plane, service time: {}", plane.created_at.elapsed().as_millis());
    }
}
