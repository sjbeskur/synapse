pub mod cfs_sys {
    #[derive(Clone, Copy, Debug, Default)]
    #[repr(C)]
    pub struct CFE_MSG_TelemetryHeader_t {
        pub bytes: [u8; 16],
    }

    #[derive(Clone, Copy, Debug, Default)]
    #[repr(C)]
    pub struct CFE_MSG_CommandHeader_t {
        pub bytes: [u8; 16],
    }
}

pub mod mission_ids {
    include!(concat!(env!("OUT_DIR"), "/synapse/mission_ids.rs"));
}

pub mod demo_msgs {
    #![allow(unused_imports)]

    use crate::cfs_sys;

    include!(concat!(env!("OUT_DIR"), "/synapse/demo_msgs.rs"));
}

fn sensor_id(name: &str) -> demo_msgs::SensorId {
    let mut bytes = [0_u8; 16];
    let src = name.as_bytes();
    let len = src.len().min(bytes.len());
    bytes[..len].copy_from_slice(&src[..len]);
    demo_msgs::SensorId { name: bytes }
}

fn main() {
    let cmd = demo_msgs::SetSensorMode {
        cfs_header: cfs_sys::CFE_MSG_CommandHeader_t::default(),
        sensor: sensor_id("imu_0"),
        mode: demo_msgs::SENSOR_MODE_SCIENCE,
    };

    let status = demo_msgs::SensorStatus {
        cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t::default(),
        sensor: sensor_id("imu_0"),
        mode: cmd.mode,
        sample_count: 42,
        temperature_c: 21.5,
    };

    println!(
        "SetSensorMode MID: 0x{:04X}",
        demo_msgs::SET_SENSOR_MODE_MID
    );
    println!("SetSensorMode CC: {}", demo_msgs::SET_SENSOR_MODE_CC);
    println!("SensorStatus MID: 0x{:04X}", demo_msgs::SENSOR_STATUS_MID);
    println!(
        "packet sizes: command={} telemetry={}",
        std::mem::size_of::<demo_msgs::SetSensorMode>(),
        std::mem::size_of::<demo_msgs::SensorStatus>()
    );
    println!(
        "status sample_count={} mode={}",
        status.sample_count, status.mode
    );
}
