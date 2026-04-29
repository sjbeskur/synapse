use crate::std_msgs;

// Message IDs
pub const ACCEL_STAMPED_MID: u16 = 0x0800;
pub const ACCEL_WITH_COVARIANCE_STAMPED_MID: u16 = 0x0801;
pub const INERTIA_STAMPED_MID: u16 = 0x0802;
pub const POINT_STAMPED_MID: u16 = 0x0803;
pub const POLYGON_STAMPED_MID: u16 = 0x0804;
pub const POSE_ARRAY_MID: u16 = 0x0805;
pub const POSE_STAMPED_MID: u16 = 0x0806;
pub const POSE_WITH_COVARIANCE_STAMPED_MID: u16 = 0x0807;
pub const QUATERNION_STAMPED_MID: u16 = 0x0808;
pub const TRANSFORM_STAMPED_MID: u16 = 0x0809;
pub const TWIST_STAMPED_MID: u16 = 0x080A;
pub const TWIST_WITH_COVARIANCE_STAMPED_MID: u16 = 0x080B;
pub const VECTOR3_STAMPED_MID: u16 = 0x080C;
pub const WRENCH_STAMPED_MID: u16 = 0x080D;

#[repr(C)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[repr(C)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[repr(C)]
pub struct Point32 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

#[repr(C)]
pub struct Accel {
    pub linear: Vector3,
    pub angular: Vector3,
}

#[repr(C)]
pub struct AccelStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub accel: Accel,
}

#[repr(C)]
pub struct AccelWithCovariance {
    pub accel: Accel,
    pub covariance: [f64; 36],
}

#[repr(C)]
pub struct AccelWithCovarianceStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub accel: AccelWithCovariance,
}

#[repr(C)]
pub struct Inertia {
    pub m: f64,
    pub com: Vector3,
    pub ixx: f64,
    pub ixy: f64,
    pub ixz: f64,
    pub iyy: f64,
    pub iyz: f64,
    pub izz: f64,
}

#[repr(C)]
pub struct InertiaStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub inertia: Inertia,
}

#[repr(C)]
pub struct PointStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub point: Point,
}

#[repr(C)]
pub struct Polygon {
    pub points: *const Point32,
}

#[repr(C)]
pub struct PolygonStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub polygon: Polygon,
}

#[repr(C)]
pub struct Pose {
    pub position: Point,
    pub orientation: Quaternion,
}

#[repr(C)]
pub struct Pose2D {
    pub x: f64,
    pub y: f64,
    pub theta: f64,
}

#[repr(C)]
pub struct PoseArray {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub poses: *const Pose,
}

#[repr(C)]
pub struct PoseStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub pose: Pose,
}

#[repr(C)]
pub struct PoseWithCovariance {
    pub pose: Pose,
    pub covariance: [f64; 36],
}

#[repr(C)]
pub struct PoseWithCovarianceStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub pose: PoseWithCovariance,
}

#[repr(C)]
pub struct QuaternionStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub quaternion: Quaternion,
}

#[repr(C)]
pub struct Transform {
    pub translation: Vector3,
    pub rotation: Quaternion,
}

#[repr(C)]
pub struct TransformStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub child_frame_id: *const u8,
    pub transform: Transform,
}

#[repr(C)]
pub struct Twist {
    pub linear: Vector3,
    pub angular: Vector3,
}

#[repr(C)]
pub struct TwistStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub twist: Twist,
}

#[repr(C)]
pub struct TwistWithCovariance {
    pub twist: Twist,
    pub covariance: [f64; 36],
}

#[repr(C)]
pub struct TwistWithCovarianceStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub twist: TwistWithCovariance,
}

#[repr(C)]
pub struct Vector3Stamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub vector: Vector3,
}

#[repr(C)]
pub struct Wrench {
    pub force: Vector3,
    pub torque: Vector3,
}

#[repr(C)]
pub struct WrenchStamped {
    pub cfs_header: cfs_sys::CFE_MSG_TelemetryHeader_t,
    pub header: std_msgs::Header,
    pub wrench: Wrench,
}

