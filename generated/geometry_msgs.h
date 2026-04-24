#pragma once
#include "cfe.h"

/* Message IDs */
#define ACCEL_STAMPED_MID  0x0800U
#define ACCEL_WITH_COVARIANCE_STAMPED_MID  0x0801U
#define INERTIA_STAMPED_MID  0x0802U
#define POINT_STAMPED_MID  0x0803U
#define POLYGON_STAMPED_MID  0x0804U
#define POSE_ARRAY_MID  0x0805U
#define POSE_STAMPED_MID  0x0806U
#define POSE_WITH_COVARIANCE_STAMPED_MID  0x0807U
#define QUATERNION_STAMPED_MID  0x0808U
#define TRANSFORM_STAMPED_MID  0x0809U
#define TWIST_STAMPED_MID  0x080AU
#define TWIST_WITH_COVARIANCE_STAMPED_MID  0x080BU
#define VECTOR3_STAMPED_MID  0x080CU
#define WRENCH_STAMPED_MID  0x080DU

typedef struct {
    double x;
    double y;
    double z;
} Vector3_t;

typedef struct {
    double x;
    double y;
    double z;
} Point_t;

typedef struct {
    float x;
    float y;
    float z;
} Point32_t;

typedef struct {
    double x;
    double y;
    double z;
    double w;
} Quaternion_t;

typedef struct {
    Vector3 linear;
    Vector3 angular;
} Accel_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Accel accel;
} AccelStamped_t;

typedef struct {
    Accel accel;
    double covariance[36];
} AccelWithCovariance_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    AccelWithCovariance accel;
} AccelWithCovarianceStamped_t;

typedef struct {
    double m;
    Vector3 com;
    double ixx;
    double ixy;
    double ixz;
    double iyy;
    double iyz;
    double izz;
} Inertia_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Inertia inertia;
} InertiaStamped_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Point point;
} PointStamped_t;

typedef struct {
    CFE_Span_t /* Point32 */ points;
} Polygon_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Polygon polygon;
} PolygonStamped_t;

typedef struct {
    Point position;
    Quaternion orientation;
} Pose_t;

typedef struct {
    double x;
    double y;
    double theta;
} Pose2D_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    CFE_Span_t /* Pose */ poses;
} PoseArray_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Pose pose;
} PoseStamped_t;

typedef struct {
    Pose pose;
    double covariance[36];
} PoseWithCovariance_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    PoseWithCovariance pose;
} PoseWithCovarianceStamped_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Quaternion quaternion;
} QuaternionStamped_t;

typedef struct {
    Vector3 translation;
    Quaternion rotation;
} Transform_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    const char* child_frame_id;
    Transform transform;
} TransformStamped_t;

typedef struct {
    Vector3 linear;
    Vector3 angular;
} Twist_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Twist twist;
} TwistStamped_t;

typedef struct {
    Twist twist;
    double covariance[36];
} TwistWithCovariance_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    TwistWithCovariance twist;
} TwistWithCovarianceStamped_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Vector3 vector;
} Vector3Stamped_t;

typedef struct {
    Vector3 force;
    Vector3 torque;
} Wrench_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header header;
    Wrench wrench;
} WrenchStamped_t;

