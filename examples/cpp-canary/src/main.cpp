#include "demo_msgs.h"

#include <algorithm>
#include <cstring>
#include <iomanip>
#include <iostream>

namespace {

demo_msgs_SensorId_t sensor_id(const char* name) {
    demo_msgs_SensorId_t id{};
    const auto len = std::min(std::strlen(name), sizeof(id.name));
    std::memcpy(id.name, name, len);
    return id;
}

} // namespace

int main() {
    demo_msgs_SetSensorMode_t cmd{};
    cmd.sensor = sensor_id("imu_0");
    cmd.mode = SENSOR_MODE_SCIENCE;

    demo_msgs_SensorStatus_t status{};
    status.sensor = sensor_id("imu_0");
    status.mode = cmd.mode;
    status.sample_count = 42;
    status.temperature_c = 21.5F;

    std::cout << "SetSensorMode MID: 0x" << std::hex << std::uppercase
              << std::setw(4) << std::setfill('0') << SET_SENSOR_MODE_MID
              << std::dec << '\n';
    std::cout << "SetSensorMode CC: " << SET_SENSOR_MODE_CC << '\n';
    std::cout << "SensorStatus MID: 0x" << std::hex << std::uppercase
              << std::setw(4) << std::setfill('0') << SENSOR_STATUS_MID
              << std::dec << '\n';
    std::cout << "packet sizes: command=" << sizeof(cmd)
              << " telemetry=" << sizeof(status) << '\n';
    std::cout << "status sample_count=" << status.sample_count
              << " mode=" << static_cast<int>(status.mode) << '\n';

    return 0;
}
