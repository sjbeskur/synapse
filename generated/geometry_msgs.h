#pragma once
#include "cfe.h"

#include "std_msgs.h"

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
} geometry_msgs_Vector3_t;

typedef struct {
    double x;
    double y;
    double z;
} geometry_msgs_Point_t;

typedef struct {
    float x;
    float y;
    float z;
} geometry_msgs_Point32_t;

typedef struct {
    double x;
    double y;
    double z;
    double w;
} geometry_msgs_Quaternion_t;

typedef struct {
    geometry_msgs_Vector3_t linear;
    geometry_msgs_Vector3_t angular;
} geometry_msgs_Accel_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Accel_t accel;
} geometry_msgs_AccelStamped_t;

typedef struct {
    geometry_msgs_Accel_t accel;
    double covariance[36];
} geometry_msgs_AccelWithCovariance_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_AccelWithCovariance_t accel;
} geometry_msgs_AccelWithCovarianceStamped_t;

typedef struct {
    double m;
    geometry_msgs_Vector3_t com;
    double ixx;
    double ixy;
    double ixz;
    double iyy;
    double iyz;
    double izz;
} geometry_msgs_Inertia_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Inertia_t inertia;
} geometry_msgs_InertiaStamped_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Point_t point;
} geometry_msgs_PointStamped_t;

typedef struct {
    CFE_Span_t /* geometry_msgs_Point32_t */ points;
} geometry_msgs_Polygon_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Polygon_t polygon;
} geometry_msgs_PolygonStamped_t;

typedef struct {
    geometry_msgs_Point_t position;
    geometry_msgs_Quaternion_t orientation;
} geometry_msgs_Pose_t;

typedef struct {
    double x;
    double y;
    double theta;
} geometry_msgs_Pose2D_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    CFE_Span_t /* geometry_msgs_Pose_t */ poses;
} geometry_msgs_PoseArray_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Pose_t pose;
} geometry_msgs_PoseStamped_t;

typedef struct {
    geometry_msgs_Pose_t pose;
    double covariance[36];
} geometry_msgs_PoseWithCovariance_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_PoseWithCovariance_t pose;
} geometry_msgs_PoseWithCovarianceStamped_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Quaternion_t quaternion;
} geometry_msgs_QuaternionStamped_t;

typedef struct {
    geometry_msgs_Vector3_t translation;
    geometry_msgs_Quaternion_t rotation;
} geometry_msgs_Transform_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    const char* child_frame_id;
    geometry_msgs_Transform_t transform;
} geometry_msgs_TransformStamped_t;

typedef struct {
    geometry_msgs_Vector3_t linear;
    geometry_msgs_Vector3_t angular;
} geometry_msgs_Twist_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Twist_t twist;
} geometry_msgs_TwistStamped_t;

typedef struct {
    geometry_msgs_Twist_t twist;
    double covariance[36];
} geometry_msgs_TwistWithCovariance_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_TwistWithCovariance_t twist;
} geometry_msgs_TwistWithCovarianceStamped_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Vector3_t vector;
} geometry_msgs_Vector3Stamped_t;

typedef struct {
    geometry_msgs_Vector3_t force;
    geometry_msgs_Vector3_t torque;
} geometry_msgs_Wrench_t;

typedef struct {
    CFE_MSG_TelemetryHeader_t Header;
    std_msgs_Header_t header;
    geometry_msgs_Wrench_t wrench;
} geometry_msgs_WrenchStamped_t;

